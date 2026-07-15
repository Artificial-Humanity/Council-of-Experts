use serde::{Deserialize, Serialize};
use thiserror::Error;
use reqwest::header::{HeaderMap, HeaderValue};
use futures_util::StreamExt;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

pub static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

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
    pub chairman: Expert,
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
    fn on_error(&self, error: &str);
}

pub trait CouncilCallback: Send + Sync {
    fn on_expert_started(&self, expert_id: &str);
    fn on_expert_chunk(&self, expert_id: &str, chunk: &str);
    fn on_expert_completed(&self, expert_id: &str, full_response: &str);
    fn on_expert_error(&self, expert_id: &str, error: &str);

    fn on_expert_critique_started(&self, expert_id: &str, round_number: u32, is_final_round: bool);
    fn on_expert_critique_chunk(&self, expert_id: &str, round_number: u32, chunk: &str);
    fn on_expert_critique_completed(&self, expert_id: &str, round_number: u32, is_final_round: bool, full_critique: &str);
    fn on_expert_critique_error(&self, expert_id: &str, round_number: u32, error: &str);

    fn on_chairman_started(&self);
    fn on_chairman_chunk(&self, chunk: &str);
    fn on_chairman_completed(&self, full_response: &str);
    fn on_chairman_error(&self, error: &str);
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

// ── OpenAI Compatible Client ──
pub struct OpenAiCompatibleClient {
    pub client: reqwest::Client,
}

impl OpenAiCompatibleClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let default_url = match expert.config.provider_type {
            ProviderType::Grok => "https://api.x.ai/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
        let base_url = expert.config.base_url.clone().unwrap_or(default_url);
        let url = format!("{}/chat/completions", base_url);
        
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", key))
                .map_err(|e| PanelError::ConfigError(e.to_string()))?);
        }

        let mut messages = vec![
            serde_json::json!({
                "role": "system",
                "content": expert.system_prompt
            })
        ];

        for msg in history {
            messages.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "content": msg.content
            }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![
                serde_json::json!({
                    "type": "text",
                    "text": prompt
                })
            ];
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": user_content
        }));

        let body = serde_json::json!({
            "model": expert.config.model_name,
            "messages": messages,
            "temperature": expert.config.temperature.unwrap_or(0.7),
            "stream": false
        });

        let res = self.client.post(&url)
            .headers(headers)
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
        let default_url = match expert.config.provider_type {
            ProviderType::Grok => "https://api.x.ai/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
        let base_url = expert.config.base_url.clone().unwrap_or(default_url);
        let url = format!("{}/chat/completions", base_url);
        
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", key))
                .map_err(|e| PanelError::ConfigError(e.to_string()))?);
        }

        let mut messages = vec![
            serde_json::json!({
                "role": "system",
                "content": expert.system_prompt
            })
        ];

        for msg in history {
            messages.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "content": msg.content
            }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![
                serde_json::json!({
                    "type": "text",
                    "text": prompt
                })
            ];
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": user_content
        }));

        let body = serde_json::json!({
            "model": expert.config.model_name,
            "messages": messages,
            "temperature": expert.config.temperature.unwrap_or(0.7),
            "stream": true
        });

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
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| PanelError::ApiError(e.to_string()))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer.drain(..=line_end).collect::<String>();
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "data: [DONE]" {
                    break;
                }

                if let Some(data_str) = trimmed.strip_prefix("data: ") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
                        if let Some(delta) = val["choices"][0]["delta"]["content"].as_str() {
                            callback.on_chunk(delta);
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

impl AnthropicClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let url = "https://api.anthropic.com/v1/messages";
        
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert("x-api-key", HeaderValue::from_str(key)
                .map_err(|e| PanelError::ConfigError(e.to_string()))?);
        }

        let mut messages = Vec::new();
        for msg in history {
            if let Role::System = msg.role {
                continue;
            }
            messages.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "content": msg.content
            }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![
                serde_json::json!({
                    "type": "text",
                    "text": prompt
                })
            ];
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": user_content
        }));

        let body = serde_json::json!({
            "model": expert.config.model_name,
            "max_tokens": 4096,
            "system": expert.system_prompt,
            "messages": messages,
            "temperature": expert.config.temperature.unwrap_or(0.7),
            "stream": false
        });

        let res = self.client.post(url)
            .headers(headers)
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
        
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        if let Some(ref key) = expert.config.api_key {
            headers.insert("x-api-key", HeaderValue::from_str(key)
                .map_err(|e| PanelError::ConfigError(e.to_string()))?);
        }

        let mut messages = Vec::new();
        for msg in history {
            if let Role::System = msg.role {
                continue;
            }
            messages.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "content": msg.content
            }));
        }

        let user_content = if attachments.is_empty() {
            serde_json::json!(prompt)
        } else {
            let mut content_parts = vec![
                serde_json::json!({
                    "type": "text",
                    "text": prompt
                })
            ];
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": user_content
        }));

        let body = serde_json::json!({
            "model": expert.config.model_name,
            "max_tokens": 4096,
            "system": expert.system_prompt,
            "messages": messages,
            "temperature": expert.config.temperature.unwrap_or(0.7),
            "stream": true
        });

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
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| PanelError::ApiError(e.to_string()))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer.drain(..=line_end).collect::<String>();
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if trimmed.starts_with("data: ") {
                    let data_str = &trimmed[6..];
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
                        if let Some(event_type) = val["type"].as_str() {
                            if event_type == "content_block_delta" {
                                if let Some(delta) = val["delta"]["text"].as_str() {
                                    callback.on_chunk(delta);
                                }
                            } else if event_type == "message_stop" {
                                break;
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

impl GeminiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for GeminiClient {
    async fn generate(&self, prompt: &str, attachments: &[Attachment], history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let api_key = expert.config.api_key.clone().unwrap_or_default();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            expert.config.model_name, api_key
        );

        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let mut contents = Vec::new();
        for msg in history {
            if let Role::System = msg.role {
                continue;
            }
            contents.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "parts": [{"text": msg.content}]
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

        contents.push(serde_json::json!({
            "role": "user",
            "parts": user_parts
        }));

        let body = serde_json::json!({
            "contents": contents,
            "systemInstruction": {
                "parts": [{"text": expert.system_prompt}]
            },
            "generationConfig": {
                "temperature": expert.config.temperature.unwrap_or(0.7)
            }
        });

        let res = self.client.post(&url)
            .headers(headers)
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
        let api_key = expert.config.api_key.clone().unwrap_or_default();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
            expert.config.model_name, api_key
        );

        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let mut contents = Vec::new();
        for msg in history {
            if let Role::System = msg.role {
                continue;
            }
            contents.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "parts": [{"text": msg.content}]
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

        contents.push(serde_json::json!({
            "role": "user",
            "parts": user_parts
        }));

        let body = serde_json::json!({
            "contents": contents,
            "systemInstruction": {
                "parts": [{"text": expert.system_prompt}]
            },
            "generationConfig": {
                "temperature": expert.config.temperature.unwrap_or(0.7)
            }
        });

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

        let mut stream = res.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| PanelError::ApiError(e.to_string()))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            let trimmed = buffer.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(arr) = val.as_array() {
                        for item in arr {
                            if let Some(delta) = item["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                callback.on_chunk(delta);
                            }
                        }
                        buffer.clear();
                    }
                }
            } else if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(delta) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        callback.on_chunk(delta);
                        buffer.clear();
                    }
                }
            } else {
                while let Some(delimiter_idx) = buffer.find("\n") {
                    let line = buffer.drain(..=delimiter_idx).collect::<String>();
                    let line_trimmed = line.trim().trim_matches(',');
                    if line_trimmed.starts_with('{') && line_trimmed.ends_with('}') {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line_trimmed) {
                            if let Some(delta) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                callback.on_chunk(delta);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ── Mock Provider ──
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
        if prompt.contains("Other panel experts have generated") {
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
    let client = reqwest::Client::new();

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
            let api_key = config.api_key.clone()
                .ok_or_else(|| PanelError::ConfigError("Missing API key".to_string()))?;

            let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=1000", api_key);
            let res = client.get(&url)
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

// Runs one discussion round for every expert in parallel and waits for all of them to finish.
// `is_opening` selects which pair of callback methods gets used: round 1 (opening statements,
// generated in isolation) reports through on_expert_*, every later round reports through
// on_expert_critique_* with the round number and whether it's the closing round.
async fn run_expert_round<F>(
    council: &Council,
    attachments: &[Attachment],
    history: &[Message],
    callback: &Arc<dyn CouncilCallback + 'static>,
    round_number: u32,
    is_final: bool,
    is_opening: bool,
    prompt_for_expert: F,
) -> Vec<(String, String)>
where
    F: Fn(&Expert) -> String,
{
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
        1, total_rounds == 1, true,
        opening_prompt_for,
    ).await;

    // Rounds 2..total_rounds: each expert reads the immediately preceding round's statements
    // and reacts (rebuttal, agreement, refinement); the last round is a closing statement.
    for round_number in 2..=total_rounds {
        let is_final = round_number == total_rounds;
        let prev = previous_round.clone();

        let prompt_for = |expert: &Expert| -> String {
            let others_str = prev.iter()
                .filter(|(id, _)| id != &expert.id)
                .map(|(id, text)| format!("- Panelist [{}]:\n{}", id, text))
                .collect::<Vec<_>>()
                .join("\n\n");

            if is_final {
                format!(
                    "{}\n\nThis is the FINAL round ({} of {}) of the panel discussion. Give your CLOSING STATEMENT. Here is what the other panelists said in the previous round:\n\n{}\n\nUser query: \"{}\"\n\nWrap up your position now — this is the last thing you'll say.",
                    style_instruction, round_number, total_rounds, others_str, prompt
                )
            } else {
                format!(
                    "{}\n\nThis is round {} of {} of the panel discussion. Here is what the other panelists said in the previous round:\n\n{}\n\nUser query: \"{}\"\n\nRespond with a rebuttal, agreement, or refinement of your position based on what you just read.",
                    style_instruction, round_number, total_rounds, others_str, prompt
                )
            }
        };

        let round_results = run_expert_round(
            council, attachments, history, &callback,
            round_number, is_final, false,
            prompt_for,
        ).await;

        if !round_results.is_empty() {
            previous_round = round_results;
        }
    }

    // Chairman Synthesis Phase — synthesizes from the closing round's statements.
    callback.on_chairman_started();

    let mut compiled_history = history.to_vec();
    for (expert_id, text) in &previous_round {
        compiled_history.push(Message {
            id: format!("msg_closing_{}", expert_id),
            role: Role::ExpertDraft { expert_id: expert_id.clone() },
            content: text.clone(),
            timestamp: 0,
        });
    }

    let synthesis_prompt = format!(
        "The original user query is: \"{}\"\n\nHere are the closing statements from the {}-round expert panel discussion:\n\n{}\n\nPlease synthesize a final, cohesive, and comprehensive response answering the user query.",
        prompt,
        total_rounds,
        previous_round.iter().map(|(id, text)| format!("- Expert [{}]:\n{}", id, text)).collect::<Vec<_>>().join("\n\n")
    );

    struct ChairmanStreamProxy {
        cb: Arc<dyn CouncilCallback + 'static>,
        full_text: Mutex<String>,
    }

    impl StreamCallback for ChairmanStreamProxy {
        fn on_chunk(&self, chunk: &str) {
            self.cb.on_chairman_chunk(chunk);
            if let Ok(mut text) = self.full_text.lock() {
                text.push_str(chunk);
            }
        }
        fn on_error(&self, error: &str) {
            self.cb.on_chairman_error(error);
        }
    }

    let proxy = ChairmanStreamProxy {
        cb: callback.clone(),
        full_text: Mutex::new(String::new()),
    };

    let chairman_provider = get_provider(&council.chairman.config.provider_type);
    match chairman_provider.generate_stream(&synthesis_prompt, &[], &compiled_history, &council.chairman, &proxy).await {
        Ok(_) => {
            let final_response = {
                let text_lock = proxy.full_text.lock().unwrap();
                text_lock.clone()
            };
            callback.on_chairman_completed(&final_response);
            Ok(final_response)
        }
        Err(e) => {
            callback.on_chairman_error(&e.to_string());
            Err(e)
        }
    }
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

pub trait CodingCallback: Send + Sync {
    fn on_expert_started(&self, expert_id: &str);
    fn on_expert_chunk(&self, expert_id: &str, chunk: &str);
    fn on_expert_completed(&self, expert_id: &str, full_response: &str);
    fn on_expert_error(&self, expert_id: &str, error: &str);

    fn on_file_write(&self, path: &str);
    fn on_build_started(&self, command: &str);
    fn on_build_completed(&self, success: bool, output: &str);

    fn on_chairman_started(&self);
    fn on_chairman_chunk(&self, chunk: &str);
    fn on_chairman_completed(&self, full_response: &str);
    fn on_chairman_error(&self, error: &str);
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

    for (_expert_id, draft) in &expert_drafts {
        let edits = parse_file_edits(draft);
        for edit in edits {
            let full_file_path = std::path::Path::new(workspace_path).join(&edit.path);
            if let Some(parent) = full_file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&full_file_path, &edit.content).is_ok() {
                callback.on_file_write(&edit.path);
            }
        }
    }

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
    
    if !build_success && council.critique_rounds > 0 {
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
        
        if !revised_drafts.is_empty() {
            final_expert_drafts = revised_drafts;
            
            for (_expert_id, draft) in &final_expert_drafts {
                let edits = parse_file_edits(draft);
                for edit in edits {
                    let full_file_path = std::path::Path::new(workspace_path).join(&edit.path);
                    if let Some(parent) = full_file_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::write(&full_file_path, &edit.content).is_ok() {
                        callback.on_file_write(&edit.path);
                    }
                }
            }
            
            if !build_command.trim().is_empty() && !workspace_path.trim().is_empty() {
                callback.on_build_started(build_command);
                let (success, log) = run_build_command(workspace_path, build_command);
                build_success = success;
                build_log = log;
                callback.on_build_completed(success, &build_log);
            }
        }
    }
    
    callback.on_chairman_started();
    
    let mut compiled_history = history.to_vec();
    for (expert_id, draft) in &final_expert_drafts {
        compiled_history.push(Message {
            id: format!("msg_coding_draft_{}", expert_id),
            role: Role::ExpertDraft { expert_id: expert_id.clone() },
            content: draft.clone(),
            timestamp: 0,
        });
    }
    
    let synthesis_prompt = format!(
        "The original user query is: \"{}\"\n\nHere are the final code draft proposals generated by the active expert council:\n\n{}\n\nWorkspace build outcome (Success: {}):\n{}\n\nPlease synthesize a final, cohesive, and comprehensive report explaining the changes made, the build test results, and any recommendations.",
        prompt,
        final_expert_drafts.iter().map(|(id, draft)| format!("- Expert [{}]:\n{}", id, draft)).collect::<Vec<_>>().join("\n\n"),
        build_success,
        build_log
    );
    
    struct ChairmanStreamProxy {
        cb: Arc<dyn CodingCallback + 'static>,
        full_text: Mutex<String>,
    }
    
    impl StreamCallback for ChairmanStreamProxy {
        fn on_chunk(&self, chunk: &str) {
            self.cb.on_chairman_chunk(chunk);
            if let Ok(mut text) = self.full_text.lock() {
                text.push_str(chunk);
            }
        }
        fn on_error(&self, error: &str) {
            self.cb.on_chairman_error(error);
        }
    }
    
    let proxy = ChairmanStreamProxy {
        cb: callback.clone(),
        full_text: Mutex::new(String::new()),
    };
    
    let chairman_provider = get_provider(&council.chairman.config.provider_type);
    match chairman_provider.generate_stream(&synthesis_prompt, &[], &compiled_history, &council.chairman, &proxy).await {
        Ok(_) => {
            let final_response = {
                let text_lock = proxy.full_text.lock().unwrap();
                text_lock.clone()
            };
            callback.on_chairman_completed(&final_response);
            Ok(final_response)
        }
        Err(e) => {
            callback.on_chairman_error(&e.to_string());
            Err(e)
        }
    }
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
        fn on_expert_critique_completed(&self, expert_id: &str, _round_number: u32, _is_final_round: bool, _full_critique: &str) {
            self.completed_critiques.lock().unwrap().push(expert_id.to_string());
        }
        fn on_expert_critique_error(&self, _expert_id: &str, _round_number: u32, _error: &str) {}

        fn on_chairman_started(&self) {}
        fn on_chairman_chunk(&self, chunk: &str) {
            self.chunks.lock().unwrap().push(chunk.to_string());
        }
        fn on_chairman_completed(&self, _full_response: &str) {}
        fn on_chairman_error(&self, _error: &str) {}
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
            chairman: Expert {
                id: "chairman".to_string(),
                name: "Chairman".to_string(),
                config: mock_config.clone(),
                system_prompt: "you are the chairman".to_string(),
            },
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
        assert_eq!(started.contains(&"expert-1".to_string()), true);
        assert_eq!(started_crit.contains(&"expert-2".to_string()), true);
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
}
