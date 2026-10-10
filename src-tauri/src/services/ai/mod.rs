#[path = "runtime/auth.rs"]
pub mod auth;
#[path = "runtime/client.rs"]
pub mod client;
#[path = "runtime/config.rs"]
pub mod config;
#[path = "catalog/models.rs"]
pub mod models;
#[path = "runtime/opencode.rs"]
pub mod opencode;
#[path = "catalog/free_models.rs"]
pub mod free_models;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

pub use auth::{detect_api_key, detect_opencode_auth_key};
pub use client::{AiService, ChatMessage};
pub use config::{
    is_local_endpoint, known_providers, preset_base_url, preset_model, provider_preset,
    AiRuntimeConfig, DEFAULT_MODEL_OPENCODE, PRESET_OPENCODE,
};
pub use models::{fetch_models_with_metadata, ModelInfoDto};

#[path = "runtime/sse.rs"]
pub mod sse;

#[path = "runtime/health.rs"]
pub mod health;
