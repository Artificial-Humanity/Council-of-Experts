use thiserror::Error;
use panel_of_experts_core as core;

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
pub enum FfiProviderType {
    OpenAi,
    Anthropic,
    Gemini,
    Grok,
    LocalOpenAiCompatible,
}

#[derive(uniffi::Record)]
pub struct FfiProviderConfig {
    pub name: String,
    pub provider_type: FfiProviderType,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(uniffi::Record)]
pub struct FfiExpert {
    pub id: String,
    pub name: String,
    pub config: FfiProviderConfig,
    pub system_prompt: String,
}

#[derive(uniffi::Enum)]
pub enum FfiRole {
    User,
    Assistant,
    System,
    ExpertDraft { expert_id: String },
    ExpertCritique { expert_id: String },
}

#[derive(uniffi::Record)]
pub struct FfiMessage {
    pub id: String,
    pub role: FfiRole,
    pub content: String,
    pub timestamp: u64,
}

#[derive(uniffi::Record)]
pub struct FfiCouncil {
    pub id: String,
    pub name: String,
    pub experts: Vec<FfiExpert>,
    pub chairman: FfiExpert,
}

#[derive(Debug, Error, uniffi::Error)]
pub enum FfiPanelError {
    #[error("API request failed: {message}")]
    ApiError { message: String },
    #[error("Serialization failed: {message}")]
    SerializationError { message: String },
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

// ── Callbacks ──
#[uniffi::export(callback_interface)]
pub trait FfiStreamCallback: Send + Sync {
    fn on_chunk(&self, chunk: String);
    fn on_complete(&self);
    fn on_error(&self, error: String);
}

struct FfiCallbackProxy {
    callback: Box<dyn FfiStreamCallback>,
}

impl core::StreamCallback for FfiCallbackProxy {
    fn on_chunk(&self, chunk: &str) {
        self.callback.on_chunk(chunk.to_string());
    }

    fn on_error(&self, error: &str) {
        self.callback.on_error(error.to_string());
    }
}

// ── Mappings ──
fn map_provider_type(pt: FfiProviderType) -> core::ProviderType {
    match pt {
        FfiProviderType::OpenAi => core::ProviderType::OpenAi,
        FfiProviderType::Anthropic => core::ProviderType::Anthropic,
        FfiProviderType::Gemini => core::ProviderType::Gemini,
        FfiProviderType::Grok => core::ProviderType::Grok,
        FfiProviderType::LocalOpenAiCompatible => core::ProviderType::LocalOpenAiCompatible,
    }
}

fn map_provider_config(pc: FfiProviderConfig) -> core::ProviderConfig {
    core::ProviderConfig {
        name: pc.name,
        provider_type: map_provider_type(pc.provider_type),
        model_name: pc.model_name,
        base_url: pc.base_url,
        api_key: pc.api_key,
        temperature: pc.temperature,
    }
}

fn map_expert(e: FfiExpert) -> core::Expert {
    core::Expert {
        id: e.id,
        name: e.name,
        config: map_provider_config(e.config),
        system_prompt: e.system_prompt,
    }
}

fn map_role(r: FfiRole) -> core::Role {
    match r {
        FfiRole::User => core::Role::User,
        FfiRole::Assistant => core::Role::Assistant,
        FfiRole::System => core::Role::System,
        FfiRole::ExpertDraft { expert_id } => core::Role::ExpertDraft { expert_id },
        FfiRole::ExpertCritique { expert_id } => core::Role::ExpertCritique { expert_id },
    }
}

fn map_message(m: FfiMessage) -> core::Message {
    core::Message {
        id: m.id,
        role: map_role(m.role),
        content: m.content,
        timestamp: m.timestamp,
    }
}

fn map_error(e: core::PanelError) -> FfiPanelError {
    match e {
        core::PanelError::ApiError(msg) => FfiPanelError::ApiError { message: msg },
        core::PanelError::SerializationError(msg) => FfiPanelError::SerializationError { message: msg },
        core::PanelError::ConfigError(msg) => FfiPanelError::ConfigError { message: msg },
        core::PanelError::Unknown(msg) => FfiPanelError::Unknown { message: msg },
    }
}

// ── Exported Functions ──
#[uniffi::export]
pub fn verify_ffi_bridge() -> String {
    "FFI Bridge Verified successfully!".to_string()
}

#[uniffi::export]
pub async fn generate_expert_response(
    prompt: String,
    history: Vec<FfiMessage>,
    expert: FfiExpert,
) -> Result<String, FfiPanelError> {
    let core_expert = map_expert(expert);
    let core_history: Vec<core::Message> = history.into_iter().map(map_message).collect();
    let provider = core::get_provider(&core_expert.config.provider_type);
    
    provider.generate(&prompt, &core_history, &core_expert)
        .await
        .map_err(map_error)
}

#[uniffi::export]
pub async fn generate_expert_stream(
    prompt: String,
    history: Vec<FfiMessage>,
    expert: FfiExpert,
    callback: Box<dyn FfiStreamCallback>,
) -> Result<(), FfiPanelError> {
    let core_expert = map_expert(expert);
    let core_history: Vec<core::Message> = history.into_iter().map(map_message).collect();
    let provider = core::get_provider(&core_expert.config.provider_type);
    let proxy = FfiCallbackProxy { callback };

    provider.generate_stream(&prompt, &core_history, &core_expert, &proxy)
        .await
        .map_err(map_error)?;

    proxy.callback.on_complete();
    Ok(())
}
