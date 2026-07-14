use serde::{Deserialize, Serialize};
use thiserror::Error;

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
