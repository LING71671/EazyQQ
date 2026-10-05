use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json};

pub fn cmd_ai_config(svc: &Services, args: &Args) -> Result<(), String> {
    let cfg = svc.ai.current();

    if args.json() {
        print_json(&serde_json::json!({
            "provider": cfg.provider,
            "baseUrl": cfg.base_url,
            "model": cfg.model,
            "temperature": cfg.temperature,
            "maxContextMessages": cfg.max_context_messages,
            "requiresApiKey": cfg.requires_api_key(),
            "apiKeyConfigured": !cfg.api_key.trim().is_empty(),
        }));
        return Ok(());
    }

    hr();
    println!("大模型供应商配置（实际生效值）");
    hr();
    println!("供应商       : {}", cfg.provider);
    println!("接口地址     : {}", cfg.base_url);
    println!("模型         : {}", cfg.model);
    println!("温度         : {}", cfg.temperature);
    println!("上下文条数   : {}", cfg.max_context_messages);
    println!(
        "是否需要 Key : {}",
        if cfg.requires_api_key() { "是" } else { "否（本地端点）" }
    );
    println!(
        "Key 已配置   : {}",
        if cfg.api_key.trim().is_empty() { "否" } else { "是" }
    );
    hr();
    println!("可用供应商（含默认端点，本地端点免 Key、省 token）：");
    for (name, url, model, local) in eazyqq_lib::services::ai::known_providers() {
        println!(
            "  {:<11} {:<34} {:<20} {}",
            name,
            url,
            model,
            if local { "本地" } else { "云端" }
        );
    }
    hr();
    println!("切换示例：");
    println!("  eazyqq_cli ai-set --provider opencode");
    println!("  eazyqq_cli ai-set --provider ollama --model <model-name>");
    println!("  eazyqq_cli ai-set --provider openai --base-url http://127.0.0.1:1234/v1 --model <model-name>");
    hr();
    Ok(())
}

/// Patch the `ai` block of `app_config`.
pub fn cmd_ai_set(svc: &Services, args: &Args) -> Result<(), String> {
    let raw = svc
        .db
        .get_setting("app_config")
        .ok()
        .flatten()
        .unwrap_or_else(|| "{}".to_string());

    let mut config: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }

    let mut ai = config.get("ai").cloned().unwrap_or_else(|| serde_json::json!({}));
    if !ai.is_object() {
        ai = serde_json::json!({});
    }

    let mut changed: Vec<String> = Vec::new();
    let mut custom_provider = false;

    if let Some(v) = args.flag("provider") {
        let provider = v.trim().to_lowercase();
        ai["activeProvider"] = serde_json::json!(provider);

        // Refresh the endpoint and model from the preset unless the caller pinned them
        // in this same call. Without this, a previously stored explicit baseUrl keeps
        // winning and the provider switch silently does nothing.
        let (preset_url, preset_model) = eazyqq_lib::services::ai::provider_preset(&provider);
        match (preset_url, preset_model) {
            (Some(url), Some(model)) => {
                if args.flag("base-url").is_none() {
                    ai["baseUrl"] = serde_json::json!(url);
                }
                if args.flag("model").is_none() {
                    ai["model"] = serde_json::json!(model);
                }
            }
            _ => {
                // Custom provider: nothing to derive from, so whatever is stored stays.
                // Flag it so the user is told what still has to be filled in.
                custom_provider = true;
            }
        }
        changed.push(format!("provider={}", provider));
    }
    if let Some(v) = args.flag("base-url") {
        ai["baseUrl"] = serde_json::json!(v);
        changed.push(format!("baseUrl={}", v));
    }
    if let Some(v) = args.flag("model") {
        ai["model"] = serde_json::json!(v);
        changed.push(format!("model={}", v));
    }
    if let Some(v) = args.flag("key") {
        ai["apiKey"] = serde_json::json!(v);
        changed.push("apiKey=***".to_string());
    }
    if let Some(v) = args.flag("temperature") {
        let t: f64 = v
            .parse()
            .map_err(|_| format!("--temperature 需要数字，收到 {}", v))?;
        ai["temperature"] = serde_json::json!(t);
        changed.push(format!("temperature={}", t));
    }
    if let Some(v) = args.flag("max-context") {
        let n: i64 = v
            .parse()
            .map_err(|_| format!("--max-context 需要整数，收到 {}", v))?;
        ai["maxContextMessages"] = serde_json::json!(n);
        changed.push(format!("maxContextMessages={}", n));
    }

    if changed.is_empty() {
        return Err(
            "未指定任何变更项。可用: --provider --base-url --model --key --temperature --max-context"
                .to_string(),
        );
    }

    config["ai"] = ai;
    let serialized =
        serde_json::to_string(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    svc.db
        .set_setting("app_config", &serialized)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    tracing::info!("AI config updated: {}", changed.join(", "));

    // Re-resolve so the user sees the effective endpoint (preset vs explicit).
    let fallback_key = svc
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let effective = eazyqq_lib::services::ai::AiRuntimeConfig::from_app_config(
        config.get("ai"),
        &fallback_key,
    );

    if args.json() {
        print_json(&serde_json::json!({
            "changed": changed,
            "effective": {
                "provider": effective.provider,
                "baseUrl": effective.base_url,
                "model": effective.model,
                "requiresApiKey": effective.requires_api_key(),
            }
        }));
        return Ok(());
    }

    println!("已更新: {}", changed.join(", "));
    println!("实际生效: {}", effective.describe());
    if let Err(e) = effective.validate() {
        println!("⚠ 配置尚不完整: {}", e);
    }
    if custom_provider {
        println!("⚠ 自定义供应商没有内置默认端点，请用 --base-url 与 --model 指定。");
    }
    println!();
    println!("提示: 已运行的客户端需要重启，或在设置页保存一次配置以热生效。");
    Ok(())
}

/// Probe the usual local inference ports and report which ones really speak the
/// OpenAI-compatible protocol.
pub async fn cmd_ai_detect(svc: &Services, args: &Args) -> Result<(), String> {
    const CANDIDATES: &[(&str, &str)] = &[
        ("http://127.0.0.1:11434/v1", "Ollama"),
        ("http://127.0.0.1:1234/v1", "LM Studio"),
        ("http://127.0.0.1:8080/v1", "llama.cpp server"),
        ("http://127.0.0.1:8000/v1", "vLLM"),
        ("http://127.0.0.1:4096/v1", "OpenCode (预期不兼容)"),
    ];

    // Probe the endpoint that is actually configured first - that is the one that matters.
    let configured = svc.ai.current();
    let mut targets: Vec<(String, String)> = Vec::new();
    if !configured.base_url.trim().is_empty() {
        targets.push((
            configured.base_url.clone(),
            format!("当前配置 ({})", configured.provider),
        ));
    }
    for (url, label) in CANDIDATES {
        if !targets.iter().any(|(u, _)| u == url) {
            targets.push((url.to_string(), label.to_string()));
        }
    }

    // Two clients: loopback probes must bypass the proxy (otherwise a machine-level
    // http_proxy turns "nothing is listening" into a misleading 502 from the proxy).
    let client_direct = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;
    let client_proxied = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let mut results: Vec<serde_json::Value> = Vec::new();

    if !args.json() {
        hr();
        println!("探测大模型端点是否可用（OpenAI 兼容）");
        hr();
    }

    for (url, label) in &targets {
        let probe = format!("{}/models", url.trim_end_matches('/'));
        let client = if eazyqq_lib::services::ai::is_local_endpoint(url) {
            &client_direct
        } else {
            &client_proxied
        };
        let verdict = match client.get(&probe).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let trimmed = body.trim_start();

                if status.as_u16() == 401 || status.as_u16() == 403 {
                    // The endpoint speaks the protocol but wants credentials - still a
                    // usable endpoint, just needs a key.
                    ("需鉴权", "是 OpenAI 兼容端点，但需要 API Key".to_string())
                } else if status.is_success() && trimmed.starts_with('{') {
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(json) if json.get("data").is_some() => {
                            let count = json
                                .get("data")
                                .and_then(|d| d.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            ("可用", format!("OpenAI 兼容，检测到 {} 个模型", count))
                        }
                        _ => ("不兼容", "返回了 JSON 但不是模型列表结构".to_string()),
                    }
                } else if status.is_success() {
                    (
                        "不兼容",
                        "返回的是 HTML 页面（很可能是 Web UI，不是推理接口）".to_string(),
                    )
                } else {
                    ("不可用", format!("HTTP {}", status))
                }
            }
            Err(e) => {
                // Distinguish "nothing there" from "there but not answering".
                let detail = if e.is_connect() {
                    "端口未监听或拒绝连接".to_string()
                } else if e.is_timeout() {
                    "端口有服务但无响应（超时）".to_string()
                } else {
                    format!("连接失败: {}", e)
                };
                ("未监听", detail)
            }
        };

        if args.json() {
            results.push(serde_json::json!({
                "label": label,
                "url": url,
                "verdict": verdict.0,
                "detail": verdict.1,
            }));
        } else {
            println!("[{}] {:<24} {:<32} {}", verdict.0, label, url, verdict.1);
        }
    }

    if args.json() {
        print_json(&serde_json::json!({ "results": results }));
    } else {
        hr();
        println!("说明: 标记为「可用」或「需鉴权」的端点才能填入 ai-set --base-url。");
        println!("本地推理推荐 Ollama / LM Studio / llama.cpp / vLLM；");
        println!("OpenCode 是编码智能体框架，其 serve 端口返回 Web UI，不提供 OpenAI 兼容接口。");
        hr();
    }
    Ok(())
}

/// Probe the configured provider with a real request.
pub async fn cmd_ai_test(svc: &Services, args: &Args) -> Result<(), String> {
    let cfg = svc.ai.current();
    println!("测试目标: {}", cfg.describe());

    let start = std::time::Instant::now();
    let result = svc.ai.generate_reply(&[], "ping", "连通性自检").await;
    let latency = start.elapsed().as_millis();

    match result {
        Ok((reply, _)) => {
            if args.json() {
                print_json(&serde_json::json!({
                    "isSuccess": true,
                    "provider": cfg.provider,
                    "model": cfg.model,
                    "endpoint": cfg.base_url,
                    "latencyMs": latency,
                    "reply": reply,
                }));
                return Ok(());
            }
            hr();
            println!("连通成功 ({} ms)", latency);
            println!("回复: {}", reply);
            hr();
            Ok(())
        }
        Err(e) => {
            if args.json() {
                print_json(&serde_json::json!({
                    "isSuccess": false,
                    "provider": cfg.provider,
                    "model": cfg.model,
                    "endpoint": cfg.base_url,
                    "error": e,
                }));
                return Ok(());
            }
            Err(format!("连通失败: {}", e))
        }
    }
}
