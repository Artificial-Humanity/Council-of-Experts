use serde::{Deserialize, Serialize};
use thiserror::Error;
use reqwest::header::{HeaderMap, HeaderValue};
use futures_util::StreamExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use once_cell::sync::Lazy;

pub static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

// Connect is bounded tightly because a mistyped LAN base URL is the common failure.
// A total timeout can't be used for streams — it can't tell a healthy long answer from a
// stalled connection — so streams are bounded by the idle gap between chunks instead.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// Set by the host app to stop an in-flight council run. Streaming loops check it between
// chunks and the orchestrator checks it between rounds, so a cancel takes effect without
// waiting for the remaining rounds to burn tokens.
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn request_cancel() {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn clear_cancel() {
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

pub fn is_cancelled() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum PanelError {
    #[error("API request failed: {0}")]
    ApiError(String),
    #[error("Serialization failed: {0}")]
    SerializationError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProviderType {
    OpenAi,
    Anthropic,
    Gemini,
    Grok,
    LocalOpenAiCompatible,
    Mock,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: ProviderType,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
    // Requests reasoning/thinking traces where the provider supports it (Anthropic extended
    // thinking, Gemini thought summaries). Best-effort elsewhere; has no effect if unsupported.
    pub enable_thinking: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expert {
    pub id: String,
    pub name: String,
    pub config: ProviderConfig,
    pub system_prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub file_path: String,
    pub mime_type: String,
    pub base64_data: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
    ExpertDraft { expert_id: String },
    ExpertCritique { expert_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Council {
    pub id: String,
    pub name: String,
    pub experts: Vec<Expert>,
    // Gates the build-failure repair loop in run_agent_coding_flow. Unrelated to `rounds` below.
    pub critique_rounds: u32,
    // Total discussion rounds for run_council_flow: round 1 is the opening statement (isolated),
    // the last round is the closing statement, anything in between is a reaction round. Minimum 2.
    pub rounds: u32,
    // Soft word-count ceiling communicated to each model via an added prompt instruction.
    pub max_response_words: u32,
}

pub trait StreamCallback: Send + Sync {
    fn on_chunk(&self, chunk: &str);
    // Default no-op: only Anthropic/Gemini clients currently emit this, when the request
    // opted into thinking/reasoning traces and the provider actually returned any.
    fn on_thinking_chunk(&self, _chunk: &str) {}
    fn on_error(&self, error: &str);
}

pub trait CouncilCallback: Send + Sync {
    fn on_expert_started(&self, expert_id: &str);
    fn on_expert_chunk(&self, expert_id: &str, chunk: &str);
    fn on_expert_thinking_chunk(&self, expert_id: &str, chunk: &str);
    fn on_expert_completed(&self, expert_id: &str, full_response: &str);
    fn on_expert_error(&self, expert_id: &str, error: &str);

    fn on_expert_critique_started(&self, expert_id: &str, round_number: u32, is_final_round: bool);
    fn on_expert_critique_chunk(&self, expert_id: &str, round_number: u32, chunk: &str);
    fn on_expert_critique_thinking_chunk(&self, expert_id: &str, round_number: u32, chunk: &str);
    fn on_expert_critique_completed(&self, expert_id: &str, round_number: u32, is_final_round: bool, full_critique: &str);
    fn on_expert_critique_error(&self, expert_id: &str, round_number: u32, error: &str);
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError>;
    async fn generate_stream(
        &self,
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError>;
}

// ── Helpers for parsing roles ──
fn role_to_string(role: &Role) -> String {
    match role {
        Role::User => "user".to_string(),
        Role::Assistant | Role::ExpertDraft { .. } | Role::ExpertCritique { .. } => "assistant".to_string(),
        Role::System => "system".to_string(),
    }
}

// Flattens a council session log into a strictly alternating user/assistant transcript.
//
// Three things have to happen here, and they're coupled enough to be worth one pass:
//   * Every panelist's statement is attributed to its author. Without the label each model
//     is handed every rival's words as its own prior turn, so it "remembers" saying things
//     it never said.
//   * Consecutive same-role turns are merged. A single council round emits one assistant
//     message per expert, and the Anthropic Messages API rejects runs of same-role turns.
//   * The transcript is trimmed to start at the first user turn, which both Anthropic and
//     Gemini require.
//
// System-role entries are dropped: all three providers take the system prompt out-of-band.
fn normalize_history(history: &[Message]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    for msg in history {
        if let Role::System = msg.role {
            continue;
        }

        let role = role_to_string(&msg.role);

        // Skip anything before the conversation's first user turn.
        if out.is_empty() && role != "user" {
            continue;
        }

        let content = match &msg.role {
            Role::ExpertDraft { expert_id } | Role::ExpertCritique { expert_id } => {
                format!("[{}]: {}", expert_id, msg.content)
            }
            _ => msg.content.clone(),
        };

        match out.last_mut() {
            Some((last_role, last_content)) if *last_role == role => {
                last_content.push_str("\n\n");
                last_content.push_str(&content);
            }
            _ => out.push((role, content)),
        }
    }

    out
}

// Pulls complete lines out of a byte buffer, leaving any trailing partial line in place.
//
// The buffer is bytes rather than a String on purpose: a multi-byte UTF-8 character can
// straddle two network chunks, and decoding each chunk as it arrives replaces the halves
// with U+FFFD — visible as garbage in any non-ASCII stream.
fn drain_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = buffer.drain(..=pos).collect();
        lines.push(String::from_utf8_lossy(&line).into_owned());
    }
    lines
}

// Reads the next chunk of an SSE body, converting a cancel request or an over-long silence
// into a stop rather than hanging the round forever.
enum StreamStep {
    Chunk(Vec<u8>),
    Done,
}

async fn next_stream_chunk<S, B>(stream: &mut S, provider: &str) -> Result<StreamStep, PanelError>
where
    S: futures_util::Stream<Item = reqwest::Result<B>> + Unpin,
    B: AsRef<[u8]>,
{
    if is_cancelled() {
        return Ok(StreamStep::Done);
    }
    match tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(chunk))) => Ok(StreamStep::Chunk(chunk.as_ref().to_vec())),
        Ok(Some(Err(e))) => Err(PanelError::ApiError(e.to_string())),
        Ok(None) => Ok(StreamStep::Done),
        Err(_) => Err(PanelError::ApiError(format!(
            "{} stream stalled: no data for {} seconds",
            provider,
            STREAM_IDLE_TIMEOUT.as_secs()
        ))),
    }
}

// ── OpenAI Compatible Client ──
pub struct OpenAiCompatibleClient {
    pub client: reqwest::Client,
}

impl Default for OpenAiCompatibleClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiCompatibleClient {
    pub fn new() -> Self {
        Self { client: http_client() }
    }

    fn endpoint(expert: &Expert) -> String {
        let default_url = match expert.config.provider_type {
            ProviderType::Grok => "https://api.x.ai/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
        let base_url = expert.config.base_url.clone().unwrap_or(default_url);
        format!("{}/chat/completions", base_url)
    }

    fn headers(expert: &Expert) -> Result<HeaderMap, PanelError> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", key))
                    .map_err(|e| PanelError::ConfigError(e.to_string()))?,
            );
        }
        Ok(headers)
    }

    // Temperature is intentionally omitted: several reasoning-tier models (gpt-5,
    // gpt-5-mini, gpt-5-nano) reject any non-default value with a 400, and this app
    // never exposed temperature as a user-configurable setting anyway.
    fn body(
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        stream: bool,
    ) -> serde_json::Value {
        let mut messages = vec![serde_json::json!({
            "role": "system",
            "content": expert.system_prompt
        })];

        for (role, content) in normalize_history(history) {
            messages.push(serde_json::json!({ "role": role, "content": content }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![serde_json::json!({ "type": "text", "text": prompt })];
            for att in attachments {
                if att.mime_type.starts_with("image/") {
                    content_parts.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", att.mime_type, att.base64_data)
                        }
                    }));
                }
            }
            serde_json::json!(content_parts)
        };

        messages.push(serde_json::json!({ "role": "user", "content": user_content }));

        serde_json::json!({
            "model": expert.config.model_name,
            "messages": messages,
            "stream": stream
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let url = Self::endpoint(expert);
        let headers = Self::headers(expert)?;
        let body = Self::body(prompt, attachments, history, expert, false);

        let res = self.client.post(&url)
            .headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("OpenAI response status: {}, body: {}", status, err_text)));
        }

        let res_json: serde_json::Value = res.json()
            .await
            .map_err(|e| PanelError::SerializationError(e.to_string()))?;

        let text = res_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| PanelError::SerializationError("Failed to extract content from OpenAI response".to_string()))?;

        Ok(text.to_string())
    }

    async fn generate_stream(
        &self,
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError> {
        let url = Self::endpoint(expert);
        let headers = Self::headers(expert)?;
        let body = Self::body(prompt, attachments, history, expert, true);

        let res = self.client.post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("OpenAI stream response status: {}, body: {}", status, err_text)));
        }

        let mut stream = res.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        'outer: loop {
            match next_stream_chunk(&mut stream, "OpenAI").await? {
                StreamStep::Done => break,
                StreamStep::Chunk(chunk) => buffer.extend_from_slice(&chunk),
            }

            for line in drain_lines(&mut buffer) {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "data: [DONE]" {
                    break 'outer;
                }

                if let Some(data_str) = trimmed.strip_prefix("data: ") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
                        if let Some(delta) = val["choices"][0]["delta"]["content"].as_str() {
                            callback.on_chunk(delta);
                        }
                        // Some OpenAI-compatible backends (xAI reasoning models, DeepSeek-style
                        // APIs, some local servers) stream reasoning separately in this field.
                        if let Some(thinking) = val["choices"][0]["delta"]["reasoning_content"].as_str() {
                            callback.on_thinking_chunk(thinking);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ── Anthropic Client ──
pub struct AnthropicClient {
    pub client: reqwest::Client,
}

impl Default for AnthropicClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicClient {
    pub fn new() -> Self {
        Self { client: http_client() }
    }

    fn headers(expert: &Expert) -> Result<HeaderMap, PanelError> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(key).map_err(|e| PanelError::ConfigError(e.to_string()))?,
            );
        }
        Ok(headers)
    }

    // Temperature is intentionally omitted: several newer model aliases (e.g.
    // claude-opus-4-8, claude-sonnet-5) reject it outright with a 400, this app never
    // exposed it as a user-configurable setting, and Anthropic disallows it entirely
    // when extended thinking is enabled.
    fn body(
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        stream: bool,
    ) -> serde_json::Value {
        let mut messages = Vec::new();
        for (role, content) in normalize_history(history) {
            messages.push(serde_json::json!({ "role": role, "content": content }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![serde_json::json!({ "type": "text", "text": prompt })];
            for att in attachments {
                if att.mime_type.starts_with("image/") {
                    content_parts.push(serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": att.mime_type,
                            "data": att.base64_data
                        }
                    }));
                }
            }
            serde_json::json!(content_parts)
        };

        messages.push(serde_json::json!({ "role": "user", "content": user_content }));

        let mut body = serde_json::json!({
            "model": expert.config.model_name,
            "max_tokens": 4096,
            "system": expert.system_prompt,
            "messages": messages,
            "stream": stream
        });

        if expert.config.enable_thinking {
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": 4096
            });
            // Extended thinking needs room to think plus room to answer.
            body["max_tokens"] = serde_json::json!(8192);
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let url = "https://api.anthropic.com/v1/messages";
        let headers = Self::headers(expert)?;
        let body = Self::body(prompt, attachments, history, expert, false);

        let res = self.client.post(url)
            .headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("Anthropic response status: {}, body: {}", status, err_text)));
        }

        let res_json: serde_json::Value = res.json()
            .await
            .map_err(|e| PanelError::SerializationError(e.to_string()))?;

        let text = res_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| PanelError::SerializationError("Failed to extract content from Anthropic response".to_string()))?;

        Ok(text.to_string())
    }

    async fn generate_stream(
        &self,
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError> {
        let url = "https://api.anthropic.com/v1/messages";
        let headers = Self::headers(expert)?;
        let body = Self::body(prompt, attachments, history, expert, true);

        let res = self.client.post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("Anthropic stream status: {}, body: {}", status, err_text)));
        }

        let mut stream = res.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        'outer: loop {
            match next_stream_chunk(&mut stream, "Anthropic").await? {
                StreamStep::Done => break,
                StreamStep::Chunk(chunk) => buffer.extend_from_slice(&chunk),
            }

            for line in drain_lines(&mut buffer) {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if let Some(data_str) = trimmed.strip_prefix("data: ") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
                        if let Some(event_type) = val["type"].as_str() {
                            if event_type == "content_block_delta" {
                                if let Some(delta) = val["delta"]["text"].as_str() {
                                    callback.on_chunk(delta);
                                } else if let Some(thinking) = val["delta"]["thinking"].as_str() {
                                    callback.on_thinking_chunk(thinking);
                                }
                            } else if event_type == "message_stop" {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ── Google Gemini Client ──
pub struct GeminiClient {
    pub client: reqwest::Client,
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiClient {
    pub fn new() -> Self {
        Self { client: http_client() }
    }

    // The key goes in a header, never the query string: reqwest embeds the request URL in
    // its error strings, and those are surfaced verbatim in the UI's error pane — a failed
    // connection would otherwise put the user's API key on screen (and in any log or
    // screenshot of it).
    fn headers(config: &ProviderConfig) -> Result<HeaderMap, PanelError> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Some(ref key) = config.api_key {
            headers.insert(
                "x-goog-api-key",
                HeaderValue::from_str(key).map_err(|e| PanelError::ConfigError(e.to_string()))?,
            );
        }
        Ok(headers)
    }

    fn body(
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
    ) -> serde_json::Value {
        let mut contents = Vec::new();
        for (role, content) in normalize_history(history) {
            // Gemini names the assistant turn "model"; sending "assistant" is rejected
            // outright as an invalid argument.
            let gemini_role = if role == "assistant" { "model" } else { "user" };
            contents.push(serde_json::json!({
                "role": gemini_role,
                "parts": [{ "text": content }]
            }));
        }

        let mut user_parts = vec![serde_json::json!({ "text": prompt })];
        for att in attachments {
            if att.mime_type.starts_with("image/") {
                user_parts.push(serde_json::json!({
                    "inline_data": {
                        "mime_type": att.mime_type,
                        "data": att.base64_data
                    }
                }));
            }
        }

        contents.push(serde_json::json!({ "role": "user", "parts": user_parts }));

        let mut generation_config = serde_json::json!({
            "temperature": expert.config.temperature.unwrap_or(0.7)
        });
        if expert.config.enable_thinking {
            generation_config["thinkingConfig"] = serde_json::json!({
                "includeThoughts": true
            });
        }

        serde_json::json!({
            "contents": contents,
            "systemInstruction": {
                "parts": [{ "text": expert.system_prompt }]
            },
            "generationConfig": generation_config
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for GeminiClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            expert.config.model_name
        );
        let headers = Self::headers(&expert.config)?;
        let body = Self::body(prompt, attachments, history, expert);

        let res = self.client.post(&url)
            .headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("Gemini response status: {}, body: {}", status, err_text)));
        }

        let res_json: serde_json::Value = res.json()
            .await
            .map_err(|e| PanelError::SerializationError(e.to_string()))?;

        let text = res_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| PanelError::SerializationError("Failed to extract content from Gemini response".to_string()))?;

        Ok(text.to_string())
    }

    async fn generate_stream(
        &self,
        prompt: &str,
        attachments: &[Attachment],
        history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError> {
        // `alt=sse` switches this endpoint from a single slowly-growing JSON array (which
        // can only be parsed once the entire response has arrived) to one complete JSON
        // object per "data:" line, streamed incrementally like the other providers.
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            expert.config.model_name
        );
        let headers = Self::headers(&expert.config)?;
        let body = Self::body(prompt, attachments, history, expert);

        let res = self.client.post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| PanelError::ApiError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(PanelError::ApiError(format!("Gemini stream response status: {}, body: {}", status, err_text)));
        }

        // A candidate's content can hold several parts when thinking is enabled: "thought"
        // parts carry reasoning, everything else is the actual answer.
        fn dispatch_gemini_parts(item: &serde_json::Value, callback: &(dyn StreamCallback + 'static)) {
            if let Some(parts) = item["candidates"][0]["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str() {
                        if part["thought"].as_bool() == Some(true) {
                            callback.on_thinking_chunk(text);
                        } else {
                            callback.on_chunk(text);
                        }
                    }
                }
            }
        }

        let mut stream = res.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        loop {
            match next_stream_chunk(&mut stream, "Gemini").await? {
                StreamStep::Done => break,
                StreamStep::Chunk(chunk) => buffer.extend_from_slice(&chunk),
            }

            for line in drain_lines(&mut buffer) {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if let Some(data_str) = trimmed.strip_prefix("data: ") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
                        dispatch_gemini_parts(&val, callback);
                    }
                }
            }
        }

        Ok(())
    }
}

// ── Mock Provider ──
#[derive(Default)]
pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn generate(&self, _prompt: &str, _attachments: &[Attachment], _history: &[Message], _expert: &Expert) -> Result<String, PanelError> {
        Ok("Mock response from expert".to_string())
    }

    async fn generate_stream(
        &self,
        prompt: &str,
        _attachments: &[Attachment],
        _history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError> {
        if expert.config.enable_thinking {
            callback.on_thinking_chunk(&format!("Mock reasoning trace from {}...", expert.name));
        }
        // Markers for the two prompts that show an expert its peers' work: the discussion
        // reaction/closing rounds, and the coding flow's build-failure repair round.
        let is_reaction = prompt.contains("the other panelists said")
            || prompt.contains("Other panel experts have generated");
        if is_reaction {
            callback.on_chunk(&format!("Mock critique and revised draft from {}", expert.name));
        } else {
            callback.on_chunk(&format!("Mock initial draft from {}", expert.name));
        }
        Ok(())
    }
}

// ── Provider Factory ──
pub fn get_provider(provider_type: &ProviderType) -> Box<dyn LlmProvider> {
    match provider_type {
        ProviderType::OpenAi | ProviderType::Grok | ProviderType::LocalOpenAiCompatible => {
            Box::new(OpenAiCompatibleClient::new())
        }
        ProviderType::Anthropic => Box::new(AnthropicClient::new()),
        ProviderType::Gemini => Box::new(GeminiClient::new()),
        ProviderType::Mock => Box::new(MockProvider::new()),
    }
}

// ── Model Discovery ──
pub async fn list_models(config: &ProviderConfig) -> Result<Vec<String>, PanelError> {
    let client = http_client();

    match config.provider_type {
        ProviderType::Mock => Ok(vec!["mock-model".to_string()]),

        ProviderType::Anthropic => {
            let api_key = config.api_key.clone()
                .ok_or_else(|| PanelError::ConfigError("Missing API key".to_string()))?;

            let mut headers = HeaderMap::new();
            headers.insert("x-api-key", HeaderValue::from_str(&api_key)
                .map_err(|e| PanelError::ConfigError(e.to_string()))?);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

            let res = client.get("https://api.anthropic.com/v1/models?limit=1000")
                .headers(headers)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|e| PanelError::ApiError(e.to_string()))?;

            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                return Err(PanelError::ApiError(format!("Anthropic models list status: {}, body: {}", status, text)));
            }

            let json: serde_json::Value = res.json().await
                .map_err(|e| PanelError::SerializationError(e.to_string()))?;

            let mut ids: Vec<String> = json["data"].as_array()
                .map(|arr| arr.iter().filter_map(|m| m["id"].as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            ids.sort();
            Ok(ids)
        }

        ProviderType::Gemini => {
            if config.api_key.is_none() {
                return Err(PanelError::ConfigError("Missing API key".to_string()));
            }

            // Key in a header, not the query string — see GeminiClient::headers.
            let url = "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000";
            let res = client.get(url)
                .headers(GeminiClient::headers(config)?)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|e| PanelError::ApiError(e.to_string()))?;

            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                return Err(PanelError::ApiError(format!("Gemini models list status: {}, body: {}", status, text)));
            }

            let json: serde_json::Value = res.json().await
                .map_err(|e| PanelError::SerializationError(e.to_string()))?;

            let mut ids: Vec<String> = json["models"].as_array()
                .map(|arr| arr.iter().filter_map(|m| {
                    m["name"].as_str().map(|s| s.trim_start_matches("models/").to_string())
                }).collect())
                .unwrap_or_default();
            ids.sort();
            Ok(ids)
        }

        ProviderType::OpenAi | ProviderType::Grok | ProviderType::LocalOpenAiCompatible => {
            let default_url = match config.provider_type {
                ProviderType::Grok => "https://api.x.ai/v1".to_string(),
                ProviderType::LocalOpenAiCompatible => "http://localhost:11434/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            };
            let base_url = config.base_url.clone().unwrap_or(default_url);
            let url = format!("{}/models", base_url);

            let mut headers = HeaderMap::new();
            if let Some(ref key) = config.api_key {
                headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", key))
                    .map_err(|e| PanelError::ConfigError(e.to_string()))?);
            }

            let res = client.get(&url)
                .headers(headers)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|e| PanelError::ApiError(e.to_string()))?;

            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                return Err(PanelError::ApiError(format!("Models list status: {}, body: {}", status, text)));
            }

            let json: serde_json::Value = res.json().await
                .map_err(|e| PanelError::SerializationError(e.to_string()))?;

            let mut ids: Vec<String> = json["data"].as_array()
                .map(|arr| arr.iter().filter_map(|m| m["id"].as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            ids.sort();
            Ok(ids)
        }
    }
}

// ── Council Flow Orchestration ──

// Which round is running and how it should report through CouncilCallback: round 1 (opening
// statements, generated in isolation) reports through on_expert_*, every later round reports
// through on_expert_critique_* with the round number and whether it's the closing round.
struct RoundMeta {
    round_number: u32,
    is_final: bool,
    is_opening: bool,
}

// Runs one discussion round for every expert in parallel and waits for all of them to finish.
async fn run_expert_round<F>(
    council: &Council,
    attachments: &[Attachment],
    history: &[Message],
    callback: &Arc<dyn CouncilCallback + 'static>,
    meta: RoundMeta,
    prompt_for_expert: F,
) -> Vec<(String, String)>
where
    F: Fn(&Expert) -> String,
{
    let RoundMeta { round_number, is_final, is_opening } = meta;
    let mut handles = Vec::new();

    for expert in &council.experts {
        let expert = expert.clone();
        let round_prompt = prompt_for_expert(&expert);
        let attachments = attachments.to_vec();
        let history = history.to_vec();
        let cb = callback.clone();

        let handle = RUNTIME.spawn(async move {
            if is_opening {
                cb.on_expert_started(&expert.id);
            } else {
                cb.on_expert_critique_started(&expert.id, round_number, is_final);
            }

            struct RoundStreamProxy {
                expert_id: String,
                cb: Arc<dyn CouncilCallback + 'static>,
                full_text: Mutex<String>,
                is_opening: bool,
                round_number: u32,
            }

            impl StreamCallback for RoundStreamProxy {
                fn on_chunk(&self, chunk: &str) {
                    if self.is_opening {
                        self.cb.on_expert_chunk(&self.expert_id, chunk);
                    } else {
                        self.cb.on_expert_critique_chunk(&self.expert_id, self.round_number, chunk);
                    }
                    if let Ok(mut text) = self.full_text.lock() {
                        text.push_str(chunk);
                    }
                }
                fn on_thinking_chunk(&self, chunk: &str) {
                    if self.is_opening {
                        self.cb.on_expert_thinking_chunk(&self.expert_id, chunk);
                    } else {
                        self.cb.on_expert_critique_thinking_chunk(&self.expert_id, self.round_number, chunk);
                    }
                }
                fn on_error(&self, error: &str) {
                    if self.is_opening {
                        self.cb.on_expert_error(&self.expert_id, error);
                    } else {
                        self.cb.on_expert_critique_error(&self.expert_id, self.round_number, error);
                    }
                }
            }

            let proxy = RoundStreamProxy {
                expert_id: expert.id.clone(),
                cb: cb.clone(),
                full_text: Mutex::new(String::new()),
                is_opening,
                round_number,
            };

            let provider = get_provider(&expert.config.provider_type);
            match provider.generate_stream(&round_prompt, &attachments, &history, &expert, &proxy).await {
                Ok(_) => {
                    let full_response = {
                        let text_lock = proxy.full_text.lock().unwrap();
                        text_lock.clone()
                    };
                    if is_opening {
                        cb.on_expert_completed(&expert.id, &full_response);
                    } else {
                        cb.on_expert_critique_completed(&expert.id, round_number, is_final, &full_response);
                    }
                    Ok((expert.id.clone(), full_response))
                }
                Err(e) => {
                    if is_opening {
                        cb.on_expert_error(&expert.id, &e.to_string());
                    } else {
                        cb.on_expert_critique_error(&expert.id, round_number, &e.to_string());
                    }
                    Err(e)
                }
            }
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(pair)) => results.push(pair),
            Ok(Err(e)) => eprintln!("Expert round error: {:?}", e),
            Err(e) => eprintln!("Tokio task join error: {:?}", e),
        }
    }
    results
}

pub async fn run_council_flow(
    prompt: &str,
    attachments: &[Attachment],
    history: &[Message],
    council: &Council,
    callback: Arc<dyn CouncilCallback + 'static>,
) -> Result<String, PanelError> {
    clear_cancel();

    let total_rounds = council.rounds.max(2);
    let max_words = if council.max_response_words == 0 { 300 } else { council.max_response_words };

    let style_instruction = format!(
        "Keep your response succinct and focused — no more than approximately {} words. Do not end your response with a question unless a clarifying question is truly necessary to proceed.",
        max_words
    );

    // Round 1: opening statements, each expert answering in isolation.
    let opening_prompt_for = |_expert: &Expert| -> String {
        format!(
            "{}\n\nThis is a {}-round panel discussion. Give your OPENING STATEMENT (round 1 of {}) responding to the user's query below. You have not seen any other panelist's answer yet — answer based on your own reasoning alone.\n\nUser query: \"{}\"",
            style_instruction, total_rounds, total_rounds, prompt
        )
    };

    let mut previous_round = run_expert_round(
        council, attachments, history, &callback,
        RoundMeta { round_number: 1, is_final: total_rounds == 1, is_opening: true },
        opening_prompt_for,
    ).await;

    // An opening round where every expert failed leaves nothing to react to; continuing
    // would spend the remaining rounds asking each model to rebut an empty transcript.
    if previous_round.is_empty() {
        return Err(PanelError::ApiError(
            "No expert produced an opening statement — check provider credentials and model names.".to_string(),
        ));
    }

    // Rounds 2..total_rounds: each expert reads the immediately preceding round's statements
    // and reacts (rebuttal, agreement, refinement); the last round is a closing statement.
    for round_number in 2..=total_rounds {
        if is_cancelled() {
            break;
        }

        let is_final = round_number == total_rounds;
        let prev = previous_round.clone();

        let prompt_for = |expert: &Expert| -> String {
            let others_str = prev.iter()
                .filter(|(id, _)| id != &expert.id)
                .map(|(id, text)| format!("- Panelist [{}]:\n{}", id, text))
                .collect::<Vec<_>>()
                .join("\n\n");

            // An expert is asked to refine "your position", so it needs to be shown what
            // that position actually was — the per-round prompt is the only place it
            // appears, since each round is a fresh single-turn request.
            let own_previous = prev.iter()
                .find(|(id, _)| id == &expert.id)
                .map(|(_, text)| format!("Your own statement in the previous round:\n{}\n\n", text))
                .unwrap_or_default();

            if is_final {
                format!(
                    "{}\n\nThis is the FINAL round ({} of {}) of the panel discussion. Give your CLOSING STATEMENT.\n\n{}Here is what the other panelists said in the previous round:\n\n{}\n\nUser query: \"{}\"\n\nWrap up your position now — this is the last thing you'll say.",
                    style_instruction, round_number, total_rounds, own_previous, others_str, prompt
                )
            } else {
                format!(
                    "{}\n\nThis is round {} of {} of the panel discussion.\n\n{}Here is what the other panelists said in the previous round:\n\n{}\n\nUser query: \"{}\"\n\nRespond with a rebuttal, agreement, or refinement of your position based on what you just read.",
                    style_instruction, round_number, total_rounds, own_previous, others_str, prompt
                )
            }
        };

        let round_results = run_expert_round(
            council, attachments, history, &callback,
            RoundMeta { round_number, is_final, is_opening: false },
            prompt_for,
        ).await;

        // A round where everyone failed (rate limit, dropped network) keeps the previous
        // round as the live transcript rather than erasing the discussion so far.
        if !round_results.is_empty() {
            previous_round = round_results;
        }
    }

    // No synthesis step: the panel's closing statements are the end of the discussion.
    Ok(previous_round.iter().map(|(id, text)| format!("- Expert [{}]:\n{}", id, text)).collect::<Vec<_>>().join("\n\n"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEdit {
    pub path: String,
    pub content: String,
}

pub fn parse_file_edits(text: &str) -> Vec<FileEdit> {
    let mut edits = Vec::new();
    let start_tag = "<write_file path=\"";
    let end_tag = "</write_file>";
    
    let mut cursor = 0;
    while let Some(start_idx) = text[cursor..].find(start_tag) {
        let absolute_start = cursor + start_idx;
        let path_start = absolute_start + start_tag.len();
        if let Some(path_end_offset) = text[path_start..].find("\">") {
            let path_end = path_start + path_end_offset;
            let path = text[path_start..path_end].to_string();
            
            let content_start = path_end + 2;
            if let Some(end_offset) = text[content_start..].find(end_tag) {
                let content_end = content_start + end_offset;
                let content = text[content_start..content_end].to_string();
                
                edits.push(FileEdit { path, content });
                cursor = content_end + end_tag.len();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    edits
}

// Resolves a model-proposed path against the workspace, refusing anything that escapes it.
//
// Model output is untrusted input here, and doubly so in this app: workspace file contents
// are pasted into the prompt, so text inside a file being reviewed can steer what the model
// emits. `Path::join` offers no protection — it walks `..` happily, and joining an absolute
// path discards the workspace root entirely — so a single crafted `<write_file>` could
// otherwise land in a shell profile and run as the user.
pub fn resolve_workspace_path(workspace_path: &str, relative: &str) -> Option<PathBuf> {
    let root = std::fs::canonicalize(workspace_path).ok()?;
    let candidate = Path::new(relative);

    if candidate.is_absolute() {
        return None;
    }

    let mut resolved = root.clone();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            // ParentDir/RootDir/Prefix are the escape routes; there's no legitimate reason
            // for a workspace-relative edit to use one.
            _ => return None,
        }
    }

    // Building from Normal components alone keeps the path lexically inside the root, but a
    // symlink in the middle of it can still point somewhere else. Check the deepest part of
    // the path that exists on disk (the file itself may be about to be created).
    let mut probe = resolved.as_path();
    loop {
        if probe.exists() {
            let real = std::fs::canonicalize(probe).ok()?;
            if !real.starts_with(&root) {
                return None;
            }
            break;
        }
        probe = probe.parent()?;
    }

    Some(resolved)
}

// Writes one expert's proposed edits into the workspace, skipping any that fail the
// containment check. Returns the paths actually written.
fn apply_file_edits(
    workspace_path: &str,
    edits: &[FileEdit],
    callback: &Arc<dyn CodingCallback + 'static>,
) -> Vec<String> {
    let mut written = Vec::new();

    for edit in edits {
        let Some(full_file_path) = resolve_workspace_path(workspace_path, &edit.path) else {
            callback.on_workspace_warning(&format!(
                "Refused to write '{}': path resolves outside the workspace directory.",
                edit.path
            ));
            continue;
        };

        if let Some(parent) = full_file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full_file_path, &edit.content) {
            Ok(_) => {
                callback.on_file_write(&edit.path);
                written.push(edit.path.clone());
            }
            Err(e) => {
                callback.on_workspace_warning(&format!("Failed to write '{}': {}", edit.path, e));
            }
        }
    }

    written
}

pub fn run_build_command(workspace_path: &str, command_str: &str) -> (bool, String) {
    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };
    
    let output = std::process::Command::new(shell)
        .arg(flag)
        .arg(command_str)
        .current_dir(workspace_path)
        .output();
        
    match output {
        Ok(out) => {
            let success = out.status.success();
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = format!("{}\n{}", stdout, stderr);
            (success, combined)
        }
        Err(e) => {
            (false, format!("Failed to run build command: {}", e))
        }
    }
}

// Applies every expert's proposed edits for one round, in expert order.
//
// NOTE: experts share a single workspace, so when two of them write the same path the last
// one wins and the build then verifies that blend rather than either expert's coherent
// proposal. That is a known limitation of this first-pass flow — see Milestone 8 in
// notes/architecture-and-roadmap.md, which calls for isolated per-expert workspaces and an
// explicit selection step. Until then, collisions are at least reported rather than silent.
fn apply_round_edits(
    workspace_path: &str,
    drafts: &[(String, String)],
    callback: &Arc<dyn CodingCallback + 'static>,
) {
    let mut authors: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for (expert_id, draft) in drafts {
        let edits = parse_file_edits(draft);
        for path in apply_file_edits(workspace_path, &edits, callback) {
            if let Some(previous) = authors.insert(path.clone(), expert_id.clone()) {
                callback.on_workspace_warning(&format!(
                    "'{}' was written by [{}] and then overwritten by [{}]; only the later version was built.",
                    path, previous, expert_id
                ));
            }
        }
    }
}

pub trait CodingCallback: Send + Sync {
    fn on_expert_started(&self, expert_id: &str);
    fn on_expert_chunk(&self, expert_id: &str, chunk: &str);
    fn on_expert_completed(&self, expert_id: &str, full_response: &str);
    fn on_expert_error(&self, expert_id: &str, error: &str);

    fn on_file_write(&self, path: &str);
    // A proposed edit that was refused or failed, and why — surfaced so a rejected write
    // isn't silently invisible to the user.
    fn on_workspace_warning(&self, message: &str);
    fn on_build_started(&self, command: &str);
    fn on_build_completed(&self, success: bool, output: &str);
}

pub async fn run_agent_coding_flow(
    prompt: &str,
    workspace_path: &str,
    build_command: &str,
    attachments: &[Attachment],
    history: &[Message],
    council: &Council,
    callback: Arc<dyn CodingCallback + 'static>,
) -> Result<String, PanelError> {
    clear_cancel();

    let mut draft_handles = Vec::new();
    let system_instructions = "\n\nIMPORTANT: You are in Coding Agent Mode. You have read access to the local workspace files. If you propose creating or modifying files, you MUST write them out fully using the following XML format:\n<write_file path=\"relative/path/to/file\">\n// file contents here\n</write_file>\nMake sure to write valid code that will compile.";
    
    for expert in &council.experts {
        let expert = expert.clone();
        let prompt = prompt.to_string();
        let attachments = attachments.to_vec();
        let history = history.to_vec();
        let cb = callback.clone();
        let system_instructions = system_instructions.to_string();
        
        let handle = RUNTIME.spawn(async move {
            cb.on_expert_started(&expert.id);
            
            struct ExpertStreamProxy {
                expert_id: String,
                cb: Arc<dyn CodingCallback + 'static>,
                full_text: Mutex<String>,
            }
            
            impl StreamCallback for ExpertStreamProxy {
                fn on_chunk(&self, chunk: &str) {
                    self.cb.on_expert_chunk(&self.expert_id, chunk);
                    if let Ok(mut text) = self.full_text.lock() {
                        text.push_str(chunk);
                    }
                }
                fn on_error(&self, error: &str) {
                    self.cb.on_expert_error(&self.expert_id, error);
                }
            }
            
            let proxy = ExpertStreamProxy {
                expert_id: expert.id.clone(),
                cb: cb.clone(),
                full_text: Mutex::new(String::new()),
            };
            
            let mut modified_expert = expert.clone();
            modified_expert.system_prompt += &system_instructions;
            
            let provider = get_provider(&modified_expert.config.provider_type);
            match provider.generate_stream(&prompt, &attachments, &history, &modified_expert, &proxy).await {
                Ok(_) => {
                    let full_response = {
                        let text_lock = proxy.full_text.lock().unwrap();
                        text_lock.clone()
                    };
                    cb.on_expert_completed(&expert.id, &full_response);
                    Ok((expert.id.clone(), full_response))
                }
                Err(e) => {
                    cb.on_expert_error(&expert.id, &e.to_string());
                    Err(e)
                }
            }
        });
        
        draft_handles.push(handle);
    }
    
    let mut expert_drafts = Vec::new();
    for handle in draft_handles {
        match handle.await {
            Ok(Ok((expert_id, response))) => {
                expert_drafts.push((expert_id, response));
            }
            Ok(Err(e)) => {
                eprintln!("Expert draft error: {:?}", e);
            }
            Err(e) => {
                eprintln!("Tokio task join error: {:?}", e);
            }
        }
    }

    // Cancelling means the drafts are half-finished, so writing them to the workspace and
    // building would be worse than doing nothing.
    if is_cancelled() {
        callback.on_workspace_warning("Cancelled before applying edits — the workspace was not modified.");
        return Ok("Cancelled before any file was written.".to_string());
    }

    apply_round_edits(workspace_path, &expert_drafts, &callback);

    let mut build_success = true;
    let mut build_log = String::new();
    if !build_command.trim().is_empty() && !workspace_path.trim().is_empty() {
        callback.on_build_started(build_command);
        let (success, log) = run_build_command(workspace_path, build_command);
        build_success = success;
        build_log = log;
        callback.on_build_completed(success, &build_log);
    }

    let mut final_expert_drafts = expert_drafts.clone();
    
    if !build_success && council.critique_rounds > 0 && !is_cancelled() {
        let mut critique_handles = Vec::new();
        
        for expert in &council.experts {
            let expert = expert.clone();
            let prompt = prompt.to_string();
            let attachments = attachments.to_vec();
            let history = history.to_vec();
            let cb = callback.clone();
            let build_log = build_log.clone();
            let system_instructions = system_instructions.to_string();
            
            let other_drafts_str = expert_drafts.iter()
                .filter(|(id, _)| id != &expert.id)
                .map(|(id, draft)| format!("- Expert [{}]:\n{}", id, draft))
                .collect::<Vec<_>>()
                .join("\n\n");
            
            let critique_prompt = format!(
                "You are an expert panelist in coding agent mode. Here is the original user query: \"{}\"\n\nOther panel experts have generated the following initial drafts:\n\n{}\n\nIMPORTANT: The initial edits failed the build/test. Here is the build/compiler log output:\n{}\n\nPlease analyze these compiler or test errors, critique the other proposals, and output your updated and fully corrected code files using `<write_file path=\"relative/path/to/file\">` tags.",
                prompt,
                other_drafts_str,
                build_log
            );

            let handle = RUNTIME.spawn(async move {
                cb.on_expert_started(&format!("{}_critique", expert.id));
                
                struct CritiqueStreamProxy {
                    expert_id: String,
                    cb: Arc<dyn CodingCallback + 'static>,
                    full_text: Mutex<String>,
                }
                
                impl StreamCallback for CritiqueStreamProxy {
                    fn on_chunk(&self, chunk: &str) {
                        self.cb.on_expert_chunk(&self.expert_id, chunk);
                        if let Ok(mut text) = self.full_text.lock() {
                            text.push_str(chunk);
                        }
                    }
                    fn on_error(&self, error: &str) {
                        self.cb.on_expert_error(&self.expert_id, error);
                    }
                }
                
                let proxy = CritiqueStreamProxy {
                    expert_id: format!("{}_critique", expert.id),
                    cb: cb.clone(),
                    full_text: Mutex::new(String::new()),
                };
                
                let mut modified_expert = expert.clone();
                modified_expert.system_prompt += &system_instructions;
                
                let provider = get_provider(&modified_expert.config.provider_type);
                match provider.generate_stream(&critique_prompt, &attachments, &history, &modified_expert, &proxy).await {
                    Ok(_) => {
                        let full_response = {
                            let text_lock = proxy.full_text.lock().unwrap();
                            text_lock.clone()
                        };
                        cb.on_expert_completed(&format!("{}_critique", expert.id), &full_response);
                        Ok((expert.id.clone(), full_response))
                    }
                    Err(e) => {
                        cb.on_expert_error(&format!("{}_critique", expert.id), &e.to_string());
                        Err(e)
                    }
                }
            });
            critique_handles.push(handle);
        }

        let mut revised_drafts = Vec::new();
        for handle in critique_handles {
            match handle.await {
                Ok(Ok((expert_id, response))) => {
                    revised_drafts.push((expert_id.clone(), response.clone()));
                }
                Ok(Err(e)) => {
                    eprintln!("Expert critique error: {:?}", e);
                }
                Err(e) => {
                    eprintln!("Tokio task join error: {:?}", e);
                }
            }
        }
        
        if !revised_drafts.is_empty() && !is_cancelled() {
            final_expert_drafts = revised_drafts;

            apply_round_edits(workspace_path, &final_expert_drafts, &callback);

            if !build_command.trim().is_empty() && !workspace_path.trim().is_empty() {
                callback.on_build_started(build_command);
                let (success, log) = run_build_command(workspace_path, build_command);
                build_success = success;
                build_log = log;
                callback.on_build_completed(success, &build_log);
            }
        }
    }
    
    // No synthesis step: file edits and the build outcome (already reported via
    // on_file_write/on_build_completed) are the end of this workflow.
    Ok(format!(
        "Build success: {}\n\n{}",
        build_success,
        final_expert_drafts.iter().map(|(id, draft)| format!("- Expert [{}]:\n{}", id, draft)).collect::<Vec<_>>().join("\n\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCouncilCallback {
        started_experts: Arc<Mutex<Vec<String>>>,
        completed_experts: Arc<Mutex<Vec<String>>>,
        started_critiques: Arc<Mutex<Vec<String>>>,
        completed_critiques: Arc<Mutex<Vec<String>>>,
        chunks: Arc<Mutex<Vec<String>>>,
    }

    impl CouncilCallback for MockCouncilCallback {
        fn on_expert_started(&self, expert_id: &str) {
            self.started_experts.lock().unwrap().push(expert_id.to_string());
        }
        fn on_expert_chunk(&self, _expert_id: &str, chunk: &str) {
            self.chunks.lock().unwrap().push(chunk.to_string());
        }
        fn on_expert_thinking_chunk(&self, _expert_id: &str, _chunk: &str) {}
        fn on_expert_completed(&self, expert_id: &str, _full_response: &str) {
            self.completed_experts.lock().unwrap().push(expert_id.to_string());
        }
        fn on_expert_error(&self, _expert_id: &str, _error: &str) {}

        fn on_expert_critique_started(&self, expert_id: &str, _round_number: u32, _is_final_round: bool) {
            self.started_critiques.lock().unwrap().push(expert_id.to_string());
        }
        fn on_expert_critique_chunk(&self, _expert_id: &str, _round_number: u32, chunk: &str) {
            self.chunks.lock().unwrap().push(chunk.to_string());
        }
        fn on_expert_critique_thinking_chunk(&self, _expert_id: &str, _round_number: u32, _chunk: &str) {}
        fn on_expert_critique_completed(&self, expert_id: &str, _round_number: u32, _is_final_round: bool, _full_critique: &str) {
            self.completed_critiques.lock().unwrap().push(expert_id.to_string());
        }
        fn on_expert_critique_error(&self, _expert_id: &str, _round_number: u32, _error: &str) {}
    }

    #[test]
    fn test_role_to_string() {
        assert_eq!(role_to_string(&Role::User), "user");
        assert_eq!(role_to_string(&Role::System), "system");
        assert_eq!(role_to_string(&Role::Assistant), "assistant");
        assert_eq!(role_to_string(&Role::ExpertDraft { expert_id: "test".to_string() }), "assistant");
        assert_eq!(role_to_string(&Role::ExpertCritique { expert_id: "test".to_string() }), "assistant");
    }

    #[test]
    fn test_get_provider_factory() {
        let _provider = get_provider(&ProviderType::OpenAi);
    }

    #[tokio::test]
    async fn test_run_council_flow() {
        let mock_config = ProviderConfig {
            name: "Mock Provider".to_string(),
            provider_type: ProviderType::Mock,
            model_name: "mock-model".to_string(),
            base_url: None,
            api_key: None,
            temperature: None,
            enable_thinking: false,
        };

        let council = Council {
            id: "test-council".to_string(),
            name: "Test Council".to_string(),
            experts: vec![
                Expert {
                    id: "expert-1".to_string(),
                    name: "Expert 1".to_string(),
                    config: mock_config.clone(),
                    system_prompt: "you are expert 1".to_string(),
                },
                Expert {
                    id: "expert-2".to_string(),
                    name: "Expert 2".to_string(),
                    config: mock_config.clone(),
                    system_prompt: "you are expert 2".to_string(),
                },
            ],
            critique_rounds: 1,
            rounds: 2,
            max_response_words: 300,
        };

        let callback = Arc::new(MockCouncilCallback {
            started_experts: Arc::new(Mutex::new(Vec::new())),
            completed_experts: Arc::new(Mutex::new(Vec::new())),
            started_critiques: Arc::new(Mutex::new(Vec::new())),
            completed_critiques: Arc::new(Mutex::new(Vec::new())),
            chunks: Arc::new(Mutex::new(Vec::new())),
        });

        let result = run_council_flow("hello", &[], &[], &council, callback.clone()).await;
        assert!(result.is_ok());
        
        let started = callback.started_experts.lock().unwrap();
        let completed = callback.completed_experts.lock().unwrap();
        let started_crit = callback.started_critiques.lock().unwrap();
        let completed_crit = callback.completed_critiques.lock().unwrap();
        
        assert_eq!(started.len(), 2);
        assert_eq!(completed.len(), 2);
        assert_eq!(started_crit.len(), 2);
        assert_eq!(completed_crit.len(), 2);
        assert!(started.contains(&"expert-1".to_string()));
        assert!(started_crit.contains(&"expert-2".to_string()));
    }

    // A second turn is where the history bugs lived: the council appends one message per
    // expert per round, so turn two replays a run of same-role messages that Anthropic
    // rejects and Gemini can't name. This drives that shape through the whole flow.
    #[tokio::test]
    async fn test_run_council_flow_with_prior_turn_history() {
        let mock_config = ProviderConfig {
            name: "Mock Provider".to_string(),
            provider_type: ProviderType::Mock,
            model_name: "mock-model".to_string(),
            base_url: None,
            api_key: None,
            temperature: None,
            enable_thinking: false,
        };

        let council = Council {
            id: "test-council".to_string(),
            name: "Test Council".to_string(),
            experts: vec![
                Expert {
                    id: "expert-1".to_string(),
                    name: "Expert 1".to_string(),
                    config: mock_config.clone(),
                    system_prompt: "you are expert 1".to_string(),
                },
                Expert {
                    id: "expert-2".to_string(),
                    name: "Expert 2".to_string(),
                    config: mock_config.clone(),
                    system_prompt: "you are expert 2".to_string(),
                },
            ],
            critique_rounds: 0,
            rounds: 3,
            max_response_words: 300,
        };

        let history = vec![
            msg(Role::User, "first turn question"),
            msg(Role::ExpertDraft { expert_id: "expert-1".to_string() }, "opening one"),
            msg(Role::ExpertDraft { expert_id: "expert-2".to_string() }, "opening two"),
            msg(Role::ExpertCritique { expert_id: "expert-1".to_string() }, "closing one"),
            msg(Role::ExpertCritique { expert_id: "expert-2".to_string() }, "closing two"),
        ];

        let normalized = normalize_history(&history);
        assert_eq!(normalized.len(), 2, "four expert turns collapse onto one user turn");
        assert!(normalized[1].1.contains("[expert-1]: opening one"));
        assert!(normalized[1].1.contains("[expert-2]: closing two"));

        let callback = Arc::new(MockCouncilCallback {
            started_experts: Arc::new(Mutex::new(Vec::new())),
            completed_experts: Arc::new(Mutex::new(Vec::new())),
            started_critiques: Arc::new(Mutex::new(Vec::new())),
            completed_critiques: Arc::new(Mutex::new(Vec::new())),
            chunks: Arc::new(Mutex::new(Vec::new())),
        });

        let result = run_council_flow("second turn question", &[], &history, &council, callback.clone()).await;
        assert!(result.is_ok());

        // Two experts across the opening round, then two more rounds of reactions.
        assert_eq!(callback.completed_experts.lock().unwrap().len(), 2);
        assert_eq!(callback.completed_critiques.lock().unwrap().len(), 4);

        // Rounds 2+ must be recognised as reaction rounds, not repeated openings.
        let chunks = callback.chunks.lock().unwrap();
        assert_eq!(
            chunks.iter().filter(|c| c.contains("critique and revised draft")).count(),
            4
        );
    }

    #[tokio::test]
    async fn test_run_council_flow_errors_when_no_expert_responds() {
        // A council with no experts stands in for "every expert failed": the opening round
        // yields nothing, so there is no transcript for later rounds to react to.
        let council = Council {
            id: "empty".to_string(),
            name: "Empty Council".to_string(),
            experts: vec![],
            critique_rounds: 0,
            rounds: 3,
            max_response_words: 300,
        };

        let callback = Arc::new(MockCouncilCallback {
            started_experts: Arc::new(Mutex::new(Vec::new())),
            completed_experts: Arc::new(Mutex::new(Vec::new())),
            started_critiques: Arc::new(Mutex::new(Vec::new())),
            completed_critiques: Arc::new(Mutex::new(Vec::new())),
            chunks: Arc::new(Mutex::new(Vec::new())),
        });

        let result = run_council_flow("hello", &[], &[], &council, callback.clone()).await;

        assert!(result.is_err(), "a silent success would bill the user for an empty discussion");
        assert!(callback.started_critiques.lock().unwrap().is_empty(), "later rounds must not run");
    }

    #[test]
    fn test_parse_file_edits() {
        let sample_text = "Some intro text.\n<write_file path=\"src/foo.rs\">\nfn foo() {}\n</write_file>\nOther text.\n<write_file path=\"tests/bar.rs\">\n#[test]\nfn bar() {}\n</write_file>\nTrailing text.";
        let edits = parse_file_edits(sample_text);
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].path, "src/foo.rs");
        assert_eq!(edits[0].content, "\nfn foo() {}\n");
        assert_eq!(edits[1].path, "tests/bar.rs");
        assert_eq!(edits[1].content, "\n#[test]\nfn bar() {}\n");
    }

    #[test]
    fn test_parse_file_edits_ignores_unclosed_tag() {
        let text = "<write_file path=\"src/ok.rs\">\ndone\n</write_file>\n<write_file path=\"src/truncated.rs\">\nfn incomplete(";
        let edits = parse_file_edits(text);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].path, "src/ok.rs");
    }

    fn msg(role: Role, content: &str) -> Message {
        Message {
            id: "id".to_string(),
            role,
            content: content.to_string(),
            timestamp: 0,
        }
    }

    #[test]
    fn test_normalize_history_merges_consecutive_expert_turns() {
        let history = vec![
            msg(Role::User, "what is the best database?"),
            msg(Role::ExpertDraft { expert_id: "expert-1".to_string() }, "Postgres."),
            msg(Role::ExpertDraft { expert_id: "expert-2".to_string() }, "SQLite."),
            msg(Role::User, "why?"),
        ];

        let normalized = normalize_history(&history);

        // The two expert turns collapse into one assistant turn so roles alternate, and
        // each keeps its author label.
        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[0].0, "user");
        assert_eq!(normalized[1].0, "assistant");
        assert_eq!(normalized[1].1, "[expert-1]: Postgres.\n\n[expert-2]: SQLite.");
        assert_eq!(normalized[2].0, "user");

        for pair in normalized.windows(2) {
            assert_ne!(pair[0].0, pair[1].0, "roles must alternate");
        }
    }

    #[test]
    fn test_normalize_history_drops_system_and_leading_assistant() {
        let history = vec![
            msg(Role::System, "ignore me"),
            msg(Role::Assistant, "stray leading assistant turn"),
            msg(Role::User, "hello"),
        ];

        let normalized = normalize_history(&history);

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].0, "user");
        assert_eq!(normalized[0].1, "hello");
    }

    #[test]
    fn test_drain_lines_preserves_utf8_split_across_chunks() {
        let mut buffer: Vec<u8> = Vec::new();
        let text = "data: 日本語\n";
        let bytes = text.as_bytes();

        // Split mid-character: the second byte of a three-byte code point.
        let split = 7;
        buffer.extend_from_slice(&bytes[..split]);
        assert!(drain_lines(&mut buffer).is_empty(), "partial line must stay buffered");

        buffer.extend_from_slice(&bytes[split..]);
        let lines = drain_lines(&mut buffer);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].trim(), "data: 日本語");
        assert!(!lines[0].contains('\u{FFFD}'), "no replacement characters");
    }

    #[test]
    fn test_drain_lines_keeps_trailing_partial() {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend_from_slice(b"first\nsecond\nthir");

        let lines = drain_lines(&mut buffer);

        assert_eq!(lines, vec!["first\n".to_string(), "second\n".to_string()]);
        assert_eq!(buffer, b"thir");
    }

    #[test]
    fn test_resolve_workspace_path_rejects_escapes() {
        let root = std::env::temp_dir().join(format!("coe-path-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root_str = root.to_str().unwrap();

        // Ordinary relative paths resolve inside the workspace, including new files.
        let ok = resolve_workspace_path(root_str, "src/new_file.rs").unwrap();
        assert!(ok.starts_with(std::fs::canonicalize(&root).unwrap()));
        assert!(resolve_workspace_path(root_str, "./src/other.rs").is_some());

        // Traversal and absolute paths are refused.
        assert!(resolve_workspace_path(root_str, "../escaped.rs").is_none());
        assert!(resolve_workspace_path(root_str, "src/../../escaped.rs").is_none());
        assert!(resolve_workspace_path(root_str, "/etc/passwd").is_none());
        assert!(resolve_workspace_path(root_str, "/Users/someone/.zshenv").is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_resolve_workspace_path_rejects_symlink_escape() {
        let base = std::env::temp_dir().join(format!("coe-symlink-test-{}", std::process::id()));
        let root = base.join("workspace");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
            let resolved = resolve_workspace_path(root.to_str().unwrap(), "escape/evil.rs");
            assert!(resolved.is_none(), "a symlink pointing outside the workspace must be refused");
        }

        std::fs::remove_dir_all(&base).ok();
    }
}
