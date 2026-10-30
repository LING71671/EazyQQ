use serde_json::json;

#[test]
fn explicitly_blank_opencode_key_does_not_reuse_another_saved_key() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({
            "activeProvider": "opencode", "apiKey": "",
            "providers": {"opencode": {"apiKey": "previous-paid-key"}}
        })),
        "unrelated-fallback",
    );
    assert!(cfg.api_key.is_empty());
    assert!(!cfg.requires_api_key());
}

#[test]
fn explicit_opencode_key_remains_a_configured_choice() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({
            "activeProvider": "opencode", "apiKey": "selected-key"
        })),
        "unrelated-fallback",
    );
    assert_eq!(cfg.api_key, "selected-key");
}

use super::client::AiService;
use super::config::{
    known_providers, provider_preset, AiRuntimeConfig, DEFAULT_MODEL_AMD, DEFAULT_MODEL_DEEPSEEK,
    DEFAULT_MODEL_NVIDIA, DEFAULT_MODEL_OPENCODE, PRESET_AMD, PRESET_DEEPSEEK, PRESET_NVIDIA,
    PRESET_OPENCODE,
};

#[test]
fn new_installation_defaults_to_official_deepseek_with_an_explicit_key_requirement() {
    let cfg = AiRuntimeConfig::default();
    assert_eq!(cfg.provider, "deepseek");
    assert_eq!(cfg.base_url, PRESET_DEEPSEEK);
    assert_eq!(cfg.model, DEFAULT_MODEL_DEEPSEEK);
    assert!(cfg.requires_api_key());
    assert!(cfg.api_key.is_empty());
    let persisted = crate::services::infra::config::default_app_config();
    assert_eq!(persisted["ai"]["activeProvider"], cfg.provider);
    assert_eq!(persisted["ai"]["model"], cfg.model);
    assert_eq!(persisted["ai"]["baseUrl"], cfg.base_url);
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
        "fallback_test_key",
    );
    assert_eq!(cfg.api_key, "fallback_test_key");
}

#[test]
fn config_api_key_wins_over_the_fallback() {
    let cfg = AiRuntimeConfig::from_app_config(
        Some(&json!({ "activeProvider": "custom", "apiKey": "explicit_test_key" })),
        "fallback_test_key",
    );
    assert_eq!(cfg.api_key, "explicit_test_key");
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
    assert_eq!(service.model(), DEFAULT_MODEL_DEEPSEEK);

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
    assert_eq!(model.as_deref(), Some(DEFAULT_MODEL_OPENCODE));
    let ids: Vec<String> = known_providers().into_iter().map(|(id, ..)| id).collect();
    assert!(ids.contains(&"opencode".to_string()));
}

#[test]
fn a_fresh_default_never_adopts_an_unrelated_provider_key() {
    let config = AiRuntimeConfig::from_app_config(None, "another_provider_fixture_key");
    assert_ne!(config.api_key, "another_provider_fixture_key");
}

#[test]
fn official_presets_require_separate_keys_and_keep_opencode_after_trial_choices() {
    let providers = known_providers();
    let ids: Vec<&str> = providers.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, ["deepseek", "nvidia", "amd", "opencode"]);
    assert!(providers.iter().all(|(_, _, _, local)| !local));
    for (provider, url, model) in [
        ("deepseek", PRESET_DEEPSEEK, DEFAULT_MODEL_DEEPSEEK),
        ("nvidia", PRESET_NVIDIA, DEFAULT_MODEL_NVIDIA),
        ("amd", PRESET_AMD, DEFAULT_MODEL_AMD),
    ] {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(
                &json!({"activeProvider":provider,"apiKey":"","providers":{"opencode":{"apiKey":"unrelated-key"}}}),
            ),
            "unrelated-fallback",
        );
        assert_eq!(cfg.base_url, url);
        assert_eq!(cfg.model, model);
        assert!(cfg.api_key.is_empty());
        assert!(cfg.requires_api_key());
        assert!(!cfg.uses_opencode_runtime());
    }
}

#[test]
fn saved_relay_model_endpoint_and_key_are_preserved_despite_new_presets() {
    for provider in ["openai", "custom", "opencode", "deepseek"] {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(
                &json!({"activeProvider":provider,"baseUrl":"https://relay.example.invalid/v1","model":"dsv4.1f","apiKey":"saved-relay-key","providers":{"deepseek":{"baseUrl":PRESET_DEEPSEEK,"model":DEFAULT_MODEL_DEEPSEEK,"apiKey":"other-saved-key"}}}),
            ),
            "unrelated-fallback",
        );
        assert_eq!(cfg.provider, provider);
        assert_eq!(cfg.base_url, "https://relay.example.invalid/v1");
        assert_eq!(cfg.model, "dsv4.1f");
        assert_eq!(cfg.api_key, "saved-relay-key");
        assert!(!cfg.uses_opencode_runtime());
    }
}
