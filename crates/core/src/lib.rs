use serde::{Deserialize, Serialize};
use thiserror::Error;
use reqwest::header::{HeaderMap, HeaderValue};
use futures_util::StreamExt;

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
}

pub trait StreamCallback: Send + Sync {
    fn on_chunk(&self, chunk: &str);
    fn on_error(&self, error: &str);
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, prompt: &str, history: &[Message], expert: &Expert) -> Result<String, PanelError>;
    async fn generate_stream(
        &self,
        prompt: &str,
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
    async fn generate(&self, prompt: &str, history: &[Message], expert: &Expert) -> Result<String, PanelError> {
        let base_url = expert.config.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string());
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
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
        history: &[Message],
        expert: &Expert,
        callback: &(dyn StreamCallback + 'static),
    ) -> Result<(), PanelError> {
        let base_url = expert.config.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string());
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
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
    async fn generate(&self, prompt: &str, history: &[Message], expert: &Expert) -> Result<String, PanelError> {
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
            // Anthropic doesn't support "system" role inside the messages array,
            // it must be passed in the root "system" parameter.
            if let Role::System = msg.role {
                continue;
            }
            messages.push(serde_json::json!({
                "role": role_to_string(&msg.role),
                "content": msg.content
            }));
        }

        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
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

        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
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
    async fn generate(&self, prompt: &str, history: &[Message], expert: &Expert) -> Result<String, PanelError> {
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

        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{"text": prompt}]
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

        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{"text": prompt}]
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
                    }
                    buffer.clear();
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

// ── Provider Factory ──
pub fn get_provider(provider_type: &ProviderType) -> Box<dyn LlmProvider> {
    match provider_type {
        ProviderType::OpenAi | ProviderType::Grok | ProviderType::LocalOpenAiCompatible => {
            Box::new(OpenAiCompatibleClient::new())
        }
        ProviderType::Anthropic => Box::new(AnthropicClient::new()),
        ProviderType::Gemini => Box::new(GeminiClient::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // factory verification only
    }
}
