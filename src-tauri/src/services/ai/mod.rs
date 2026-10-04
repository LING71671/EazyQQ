pub mod auth;
pub mod client;
pub mod config;
pub mod models;

#[cfg(test)]
mod tests;

pub use auth::{detect_api_key, detect_opencode_auth_key};
pub use client::{AiService, ChatMessage};
pub use config::{
    is_local_endpoint, known_providers, preset_base_url, preset_model, provider_preset,
    AiRuntimeConfig, DEFAULT_MODEL_OPENCODE, PRESET_OPENCODE,
};
pub use models::fetch_models_from_endpoint;
