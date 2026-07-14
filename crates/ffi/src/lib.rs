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

#[uniffi::export]
pub fn verify_ffi_bridge() -> String {
    "FFI Bridge Verified successfully!".to_string()
}
