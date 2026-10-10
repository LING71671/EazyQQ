use super::config::AiRuntimeConfig;

pub struct InferenceHealth {
    pub verified: Option<bool>,
    pub detail: String,
}

struct Evidence {
    config: AiRuntimeConfig,
    sequence: u64,
    succeeded: bool,
    detail: String,
}

#[derive(Default)]
pub struct InferenceEvidence {
    active: Option<Evidence>,
    preview: Option<Evidence>,
}

pub fn same_target(left: &AiRuntimeConfig, right: &AiRuntimeConfig) -> bool {
    let model = |cfg: &AiRuntimeConfig| {
        let value = cfg.model.trim();
        if cfg.uses_opencode_runtime() { value.strip_prefix("opencode/").unwrap_or(value).to_string() }
        else { value.to_string() }
    };
    left.provider == right.provider && left.uses_opencode_runtime() == right.uses_opencode_runtime() && model(left) == model(right)
        && (left.uses_opencode_runtime() || left.base_url.trim_end_matches('/') == right.base_url.trim_end_matches('/'))
        && left.api_key.trim() == right.api_key.trim()
}

impl InferenceEvidence {
    pub fn record(&mut self, current: &AiRuntimeConfig, tested: &AiRuntimeConfig, sequence: u64, error: Option<&str>) {
        let slot = if same_target(current, tested) { &mut self.active } else { &mut self.preview };
        if slot.as_ref().is_some_and(|evidence| evidence.sequence > sequence) { return; }
        let detail = match error {
            None => format!("模型 {} 已通过推理验证", tested.model),
            Some(error) => {
                let safe = if tested.api_key.is_empty() { error.to_string() } else { error.replace(&tested.api_key, "[已隐藏]") };
                format!("模型 {} 推理失败：{}", tested.model, safe)
            }
        };
        *slot = Some(Evidence { config: tested.clone(), sequence, succeeded: error.is_none(), detail });
    }

    pub fn reconfigure(&mut self, previous: &AiRuntimeConfig, next: &AiRuntimeConfig) {
        if same_target(previous, next) { return; }
        self.active = if self.preview.as_ref().is_some_and(|e| same_target(&e.config, next)) { self.preview.take() } else { None };
    }

    pub fn current(&self, config: &AiRuntimeConfig) -> InferenceHealth {
        match self.active.as_ref().filter(|e| same_target(&e.config, config)) {
            Some(e) => InferenceHealth { verified: Some(e.succeeded), detail: e.detail.clone() },
            None => InferenceHealth { verified: None, detail: format!("当前生效模型 {} 尚未验证推理能力", config.model) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(model: &str) -> AiRuntimeConfig { AiRuntimeConfig { model: model.into(), ..Default::default() } }

    #[test]
    fn passive_reads_preserve_verified_success() {
        let cfg = config("a"); let mut cache = InferenceEvidence::default();
        cache.record(&cfg, &cfg, 1, None);
        for _ in 0..10 { assert_eq!(cache.current(&cfg).verified, Some(true)); }
    }
    #[test]
    fn preview_does_not_change_active_health_but_applies_when_saved() {
        let active = config("a"); let preview = config("b"); let mut cache = InferenceEvidence::default();
        cache.record(&active, &active, 1, None); cache.record(&active, &preview, 2, Some("failed"));
        assert_eq!(cache.current(&active).verified, Some(true));
        cache.reconfigure(&active, &preview);
        assert_eq!(cache.current(&preview).verified, Some(false));
    }
    #[test]
    fn new_target_and_credentials_require_new_evidence() {
        let cfg = config("a"); let mut next = cfg.clone(); next.api_key = "changed".into();
        let mut cache = InferenceEvidence::default(); cache.record(&cfg, &cfg, 1, None);
        cache.reconfigure(&cfg, &next); assert_eq!(cache.current(&next).verified, None);
    }
    #[test]
    fn late_previous_request_cannot_overwrite_newer_success() {
        let cfg = config("a"); let mut cache = InferenceEvidence::default();
        cache.record(&cfg, &cfg, 2, None); cache.record(&cfg, &cfg, 1, Some("late error"));
        assert_eq!(cache.current(&cfg).verified, Some(true));
    }
    #[test]
    fn repeated_save_and_native_prefix_keep_matching_proof() {
        let cfg = config("a"); let mut next = config("opencode/a"); next.temperature = 0.2;
        let mut cache = InferenceEvidence::default(); cache.record(&cfg, &cfg, 1, None);
        cache.reconfigure(&cfg, &next); assert_eq!(cache.current(&next).verified, Some(true));
    }
    #[test]
    fn native_and_proxy_routes_are_different_targets() {
        let cfg = config("a"); let mut proxy = cfg.clone(); proxy.base_url = "http://127.0.0.1:9000/v1".into();
        assert!(!same_target(&cfg, &proxy)); assert!(!same_target(&proxy, &cfg));
    }
    #[test]
    fn successful_preview_is_unconfirmed_until_explicitly_saved() {
        let cfg = config("a"); let preview = config("b"); let mut cache = InferenceEvidence::default();
        cache.record(&cfg, &preview, 1, None); assert_eq!(cache.current(&cfg).verified, None);
        cache.reconfigure(&cfg, &preview); assert_eq!(cache.current(&preview).verified, Some(true));
    }
}
