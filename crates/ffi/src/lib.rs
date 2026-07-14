use thiserror::Error;
use panel_of_experts_core as core;
use std::sync::Arc;

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
pub enum FfiProviderType {
    OpenAi,
    Anthropic,
    Gemini,
    Grok,
    LocalOpenAiCompatible,
    Mock,
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
    pub critique_rounds: u32,
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

#[uniffi::export(callback_interface)]
pub trait FfiCouncilCallback: Send + Sync {
    fn on_expert_started(&self, expert_id: String);
    fn on_expert_chunk(&self, expert_id: String, chunk: String);
    fn on_expert_completed(&self, expert_id: String, full_response: String);
    fn on_expert_error(&self, expert_id: String, error: String);
    
    fn on_expert_critique_started(&self, expert_id: String);
    fn on_expert_critique_chunk(&self, expert_id: String, chunk: String);
    fn on_expert_critique_completed(&self, expert_id: String, full_critique: String);
    fn on_expert_critique_error(&self, expert_id: String, error: String);

    fn on_chairman_started(&self);
    fn on_chairman_chunk(&self, chunk: String);
    fn on_chairman_completed(&self, full_response: String);
    fn on_chairman_error(&self, error: String);
}

struct FfiCouncilCallbackProxy {
    callback: Box<dyn FfiCouncilCallback>,
}

impl core::CouncilCallback for FfiCouncilCallbackProxy {
    fn on_expert_started(&self, expert_id: &str) {
        self.callback.on_expert_started(expert_id.to_string());
    }
    fn on_expert_chunk(&self, expert_id: &str, chunk: &str) {
        self.callback.on_expert_chunk(expert_id.to_string(), chunk.to_string());
    }
    fn on_expert_completed(&self, expert_id: &str, full_response: &str) {
        self.callback.on_expert_completed(expert_id.to_string(), full_response.to_string());
    }
    fn on_expert_error(&self, expert_id: &str, error: &str) {
        self.callback.on_expert_error(expert_id.to_string(), error.to_string());
    }
    
    fn on_expert_critique_started(&self, expert_id: &str) {
        self.callback.on_expert_critique_started(expert_id.to_string());
    }
    fn on_expert_critique_chunk(&self, expert_id: &str, chunk: &str) {
        self.callback.on_expert_critique_chunk(expert_id.to_string(), chunk.to_string());
    }
    fn on_expert_critique_completed(&self, expert_id: &str, full_critique: &str) {
        self.callback.on_expert_critique_completed(expert_id.to_string(), full_critique.to_string());
    }
    fn on_expert_critique_error(&self, expert_id: &str, error: &str) {
        self.callback.on_expert_critique_error(expert_id.to_string(), error.to_string());
    }

    fn on_chairman_started(&self) {
        self.callback.on_chairman_started();
    }
    fn on_chairman_chunk(&self, chunk: &str) {
        self.callback.on_chairman_chunk(chunk.to_string());
    }
    fn on_chairman_completed(&self, full_response: &str) {
        self.callback.on_chairman_completed(full_response.to_string());
    }
    fn on_chairman_error(&self, error: &str) {
        self.callback.on_chairman_error(error.to_string());
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
        FfiProviderType::Mock => core::ProviderType::Mock,
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

fn map_council(c: FfiCouncil) -> core::Council {
    core::Council {
        id: c.id,
        name: c.name,
        experts: c.experts.into_iter().map(map_expert).collect(),
        chairman: map_expert(c.chairman),
        critique_rounds: c.critique_rounds,
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

#[uniffi::export]
pub async fn execute_council_workflow(
    prompt: String,
    history: Vec<FfiMessage>,
    council: FfiCouncil,
    callback: Box<dyn FfiCouncilCallback>,
) -> Result<String, FfiPanelError> {
    let core_council = map_council(council);
    let core_history: Vec<core::Message> = history.into_iter().map(map_message).collect();
    let proxy = Arc::new(FfiCouncilCallbackProxy { callback });

    core::run_council_flow(&prompt, &core_history, &core_council, proxy)
        .await
        .map_err(map_error)
}
