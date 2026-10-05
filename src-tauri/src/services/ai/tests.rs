use serde_json::json;

use super::client::AiService;
use super::config::{
    known_providers, provider_preset, AiRuntimeConfig, DEFAULT_MODEL_OPENCODE, PRESET_OPENCODE,
};

#[test]
fn default_config_points_at_opencode() {
    let cfg = AiRuntimeConfig::default();
    assert_eq!(cfg.provider, "opencode");
    assert_eq!(cfg.base_url, PRESET_OPENCODE);
}

#[test]
fn explicit_base_url_and_model_override_defaults() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({
            "activeProvider": "custom",
            "baseUrl": "http://127.0.0.1:9999/v1",
            "model": "my-local-model"
        })),
        "",
    );
    assert_eq!(cfg.base_url, "http://127.0.0.1:9999/v1");
    assert_eq!(cfg.model, "my-local-model");
}

#[test]
fn local_endpoints_do_not_require_a_key() {
    for url in [
        "http://127.0.0.1:4096/v1",
        "http://localhost:11434/v1",
        "http://0.0.0.0:1234/v1",
    ] {
        let cfg = AiRuntimeConfig {
            provider: "custom".to_string(),
            base_url: url.to_string(),
            api_key: String::new(),
            ..AiRuntimeConfig::default()
        };
        assert!(!cfg.requires_api_key(), "{url} should not need a key");

        let service = AiService::new(cfg);
        assert!(service.has_api_key(), "a local endpoint must be usable");
    }
}

#[test]
fn remote_custom_endpoints_do_require_a_key() {
    let cfg = AiRuntimeConfig {
        provider: "custom".to_string(),
        base_url: "https://my-api-proxy.example.com/v1".to_string(),
        api_key: String::new(),
        ..AiRuntimeConfig::default()
    };
    assert!(cfg.requires_api_key());
    assert!(!AiService::new(cfg).has_api_key());
}

#[test]
fn fallback_key_is_used_when_the_config_has_none() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({ "activeProvider": "custom", "apiKey": "   " })),
        "sk-from-settings",
    );
    assert_eq!(cfg.api_key, "sk-from-settings");
}

#[test]
fn config_api_key_wins_over_the_fallback() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({ "activeProvider": "custom", "apiKey": "sk-explicit" })),
        "sk-from-settings",
    );
    assert_eq!(cfg.api_key, "sk-explicit");
}

#[test]
fn temperature_and_context_are_clamped() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({ "temperature": 99.0, "maxContextMessages": 100000 })),
        "",
    );
    assert_eq!(cfg.temperature, 2.0);
    assert_eq!(cfg.max_context_messages, 100);

    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({ "temperature": -5.0, "maxContextMessages": 0 })),
        "",
    );
    assert_eq!(cfg.temperature, 0.0);
    assert_eq!(cfg.max_context_messages, 1);
}

#[test]
fn reconfigure_swaps_the_active_configuration() {
    let service = AiService::new(AiRuntimeConfig::default());
    assert_eq!(service.model(), DEFAULT_MODEL_OPENCODE);

    service.reconfigure(AiRuntimeConfig {
        provider: "custom".to_string(),
        base_url: "http://127.0.0.1:11434/v1".to_string(),
        model: "local-llama".to_string(),
        ..AiRuntimeConfig::default()
    });

    assert_eq!(service.model(), "local-llama");
    assert_eq!(service.current().base_url, "http://127.0.0.1:11434/v1");
    assert!(service.has_api_key(), "local endpoint needs no key");
}

#[test]
fn opencode_is_offered_with_zen_endpoint() {
    let (url, model) = provider_preset("opencode");
    assert_eq!(url.as_deref(), Some(PRESET_OPENCODE));
    assert_eq!(model.as_deref(), None);
    let ids: Vec<String> = known_providers().into_iter().map(|(id, ..)| id).collect();
    assert!(ids.contains(&"opencode".to_string()));
}
