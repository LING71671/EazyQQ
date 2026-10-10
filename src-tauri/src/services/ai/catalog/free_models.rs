//! Current native catalogue plus explicit credential-free inference evidence.
use super::opencode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModel {
    pub id: String,
    pub name: String,
    pub state: String,
    pub detail: String,
    pub observed_at_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModelsReport {
    pub runtime_version: String,
    pub observed_at_ms: u64,
    pub catalogue_ids: Vec<String>,
    pub models: Vec<FreeModel>,
    pub credentials_used: bool,
    pub source: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModelProgress {
    pub request_id: String,
    pub completed: usize,
    pub total: usize,
    pub model: FreeModel,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn parse_catalogue(output: &str) -> Result<Vec<Value>, String> {
    let mut input = output;
    let mut values = Vec::new();
    while !input.trim().is_empty() {
        input = input.trim_start();
        let (header, tail) = input
            .split_once('\n')
            .ok_or("OpenCode 模型元数据格式不完整")?;
        let id = header
            .trim()
            .strip_prefix("opencode/")
            .ok_or("OpenCode 返回了意外的供应方")?;
        let mut parser = serde_json::Deserializer::from_str(tail).into_iter::<Value>();
        let value = parser
            .next()
            .ok_or("OpenCode 模型元数据缺失")?
            .map_err(|error| error.to_string())?;
        if value["id"].as_str() != Some(id) || value["providerID"] != "opencode" {
            return Err("OpenCode 模型标识与元数据不一致".into());
        }
        input = &tail[parser.byte_offset()..];
        values.push(value);
    }
    if values.is_empty() {
        return Err("OpenCode 当前目录没有模型，无法确认免费资格".into());
    }
    Ok(values)
}

fn zero_priced(value: &Value) -> bool {
    value["cost"]["input"].as_f64() == Some(0.0)
        && value["cost"]["output"].as_f64() == Some(0.0)
        && ["read", "write"].iter().all(|field| {
            value["cost"]["cache"][field].is_null()
                || value["cost"]["cache"][field].as_f64() == Some(0.0)
        })
}

fn failure_state(message: &str) -> &'static str {
    let message = message.to_lowercase();
    if ["deprecated", "retired", "no longer supported"]
        .iter()
        .any(|term| message.contains(term))
    {
        "retired"
    } else if [
        "api key",
        "sign in",
        "credential",
        "payment",
        "credits",
        "within opencode",
        "tool operation",
    ]
    .iter()
    .any(|term| message.contains(term))
    {
        "requires_conditions"
    } else {
        "unconfirmed"
    }
}

pub async fn catalogue(root: &Path) -> Result<Vec<Value>, String> {
    catalogue_with_key(root, None).await
}

pub async fn catalogue_with_key(root: &Path, key: Option<&str>) -> Result<Vec<Value>, String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let mut command = opencode::command()?;
    command
        .args(["models", "opencode", "--refresh", "--verbose"])
        .current_dir(root)
        .stderr(std::process::Stdio::null());
    opencode::anonymous_environment(&mut command, root);
    if let Some(key) = key.filter(|key| !key.trim().is_empty()) {
        command.env("OPENCODE_CONFIG_CONTENT",serde_json::json!({"enabled_providers":["opencode"],"provider":{"opencode":{"options":{"apiKey":key}}}}).to_string());
    }
    let output = tokio::time::timeout(std::time::Duration::from_secs(40), command.output())
        .await
        .map_err(|_| "OpenCode 目录刷新超过 40 秒，未使用旧缓存冒充检测结果".to_string())?
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("OpenCode 实时目录读取失败，请检查原生运行时与网络".into());
    }
    if output.stdout.len() > 4_000_000 {
        return Err("OpenCode 模型目录超过大小限制".into());
    }
    parse_catalogue(&String::from_utf8_lossy(&output.stdout))
}

pub async fn detect<F>(
    root: &Path,
    probe: bool,
    request_id: &str,
    mut progress: F,
) -> Result<FreeModelsReport, String>
where
    F: FnMut(FreeModelProgress) + Send,
{
    let _lock = if probe {
        Some(
            crate::services::infra::persistence::FileLock::try_acquire(
                &root.join("free-model-detection.lock"),
            )
            .map_err(|_| "当前账号已有免费模型检测，请等待完成".to_string())?,
        )
    } else {
        None
    };
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let directory = root.join(format!(
        "free-model-detection-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    let values = catalogue(&directory.join("catalogue")).await?;
    let mut version_command = opencode::command()?;
    version_command.arg("--version");
    let version = tokio::time::timeout(std::time::Duration::from_secs(5), version_command.output())
        .await
        .map_err(|_| "读取 OpenCode 版本超时")?
        .map_err(|e| e.to_string())?;
    let mut report = FreeModelsReport {
        runtime_version: String::from_utf8_lossy(&version.stdout).trim().into(),
        observed_at_ms: now(),
        catalogue_ids: values
            .iter()
            .filter_map(|v| v["id"].as_str().map(str::to_string))
            .collect(),
        models: Vec::new(),
        credentials_used: false,
        source:
            "native_opencode_catalogue_refresh_requested; anonymous_data_config_and_environment"
                .into(),
    };
    let candidates: Vec<_> = values.iter().filter(|v| zero_priced(v)).collect();
    let total = candidates.len();
    let previous = std::fs::read(root.join("free-model-report.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<FreeModelsReport>(&bytes).ok());
    for (index, value) in candidates.into_iter().enumerate() {
        let id = value["id"].as_str().unwrap().to_string();
        let mut model = FreeModel {
            name: value["name"].as_str().unwrap_or(&id).into(),
            id,
            state: "candidate".into(),
            detail: "目录标价为零；本次目录刷新尚未进行免凭据调用验证".into(),
            observed_at_ms: now(),
        };
        if value["status"] == "deprecated" || value["status"] == "retired" {
            model.state = "retired".into();
            model.detail = "原生模型元数据明确标记停用".into();
        } else if probe {
            let attempt = directory.join(format!("probe-{index}"));
            match opencode::probe_without_credentials(&attempt, &model.id).await {
                Ok(()) => {
                    model.state = "available".into();
                    model.detail = "本次隔离请求未使用账号凭据或 API Key，已返回模型文本".into();
                }
                Err(error) => {
                    model.state = failure_state(&error).into();
                    model.detail = crate::services::diagnostics::redact(&error, &[]);
                }
            }
        }
        model.observed_at_ms = now();
        report.models.push(model.clone());
        progress(FreeModelProgress {
            request_id: request_id.into(),
            completed: index + 1,
            total,
            model,
        });
    }
    if let Some(previous) = previous {
        for model in previous.models {
            if !report.catalogue_ids.contains(&model.id) {
                report.models.push(FreeModel {
                    state: "removed_from_catalogue".into(),
                    detail: "本次原生目录已没有此模型；不以网络失败推断下架".into(),
                    observed_at_ms: now(),
                    ..model
                });
            }
        }
    }
    report.observed_at_ms = now();
    if probe {
        crate::services::infra::persistence::write_json(
            &root.join("free-model-report.json"),
            &report,
        )?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn free_candidates_need_explicit_zero_prices_not_a_name_suffix() {
        assert!(zero_priced(
            &serde_json::json!({"id":"ordinary","cost":{"input":0,"output":0}})
        ));
        assert!(!zero_priced(
            &serde_json::json!({"id":"paid-free","cost":{"input":1,"output":0}})
        ));
        assert!(!zero_priced(&serde_json::json!({"id":"missing-free"})));
        assert!(!zero_priced(
            &serde_json::json!({"cost":{"input":0,"output":0,"cache":{"read":1}}})
        ));
    }
    #[test]
    fn native_records_are_bound_to_their_provider_and_exact_id() {
        let valid="opencode/plain\n{\"id\":\"plain\",\"providerID\":\"opencode\",\"cost\":{\"input\":0,\"output\":0}}\n";
        assert_eq!(parse_catalogue(valid).unwrap().len(), 1);
        assert!(parse_catalogue(&valid.replace("opencode/plain", "opencode/other")).is_err());
        assert!(parse_catalogue("opencode/partial\n{").is_err());
    }
    #[test]
    fn transient_failures_do_not_claim_retirement_or_free_access() {
        assert_eq!(
            failure_state("request timed out after 25 seconds"),
            "unconfirmed"
        );
        assert_eq!(failure_state("429 rate limited"), "unconfirmed");
        assert_eq!(failure_state("API key required"), "requires_conditions");
        assert_eq!(failure_state("model has been deprecated"), "retired");
        assert_eq!(
            failure_state("Model not found in local catalogue"),
            "unconfirmed"
        );
    }
}
