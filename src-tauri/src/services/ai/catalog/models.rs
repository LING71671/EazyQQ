use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use super::config::is_local_endpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoDto {
    pub id: String,
    pub name: String,
    pub is_free: bool,
    pub cost_input: Option<f64>,
    pub cost_output: Option<f64>,
}

fn parse_models(value: &Value, is_local: bool) -> Vec<ModelInfoDto> {
    let entries = value.get("data").and_then(Value::as_array).or_else(|| {
        if is_local {
            value.get("models").and_then(Value::as_array)
        } else {
            None
        }
    });
    let mut models: Vec<ModelInfoDto> = entries
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| {
                    if is_local {
                        item.get("name").and_then(Value::as_str)
                    } else {
                        None
                    }
                })?
                .trim();
            if id.is_empty() {
                return None;
            }
            Some(ModelInfoDto {
                id: id.into(),
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .into(),
                // A catalogue response is not evidence of a cloud key's remaining quota.
                is_free: is_local,
                cost_input: if is_local { Some(0.0) } else { None },
                cost_output: if is_local { Some(0.0) } else { None },
            })
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    models
}

/// Fetch the configured key's catalogue, without testing inference or assuming free quota.
pub async fn fetch_models_with_metadata(
    provider: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfoDto>, String> {
    let endpoint = reqwest::Url::parse(base_url).map_err(|_| "模型目录接口地址无效")?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err("模型目录接口必须是无用户名密码的 HTTP(S) 地址".into());
    }
    let uses_native_opencode = matches!(provider, "opencode" | "opencode-go")
        && endpoint.host_str() == Some("opencode.ai");
    if uses_native_opencode {
        let root = crate::services::accounts::active_data_dir().join("ai-runtime/model-catalogue");
        let values = super::free_models::catalogue_with_key(&root, api_key).await?;
        return Ok(values
            .into_iter()
            .filter(|value| !matches!(value["status"].as_str(), Some("deprecated" | "retired")))
            .filter_map(|value| {
                let id = value["id"].as_str()?;
                Some(ModelInfoDto {
                    id: id.into(),
                    name: value["name"].as_str().unwrap_or(id).into(),
                    is_free: false,
                    cost_input: value["cost"]["input"].as_f64(),
                    cost_output: value["cost"]["output"].as_f64(),
                })
            })
            .collect());
    }
    let is_local = is_local_endpoint(base_url);
    let key = api_key.map(str::trim).filter(|key| !key.is_empty());
    if matches!(provider, "deepseek" | "nvidia" | "amd") && !is_local && key.is_none() {
        return Err("请先填写当前供应方的 API Key，再获取该 Key 可访问的模型目录".into());
    }
    let builder = Client::builder().timeout(Duration::from_secs(15));
    let client = if is_local {
        builder.no_proxy()
    } else {
        builder
    }
    .build()
    .map_err(|_| "无法创建模型目录连接")?;
    let base = base_url.trim_end_matches('/');
    let url = if base.ends_with("/models") {
        base.to_owned()
    } else {
        format!("{base}/models")
    };
    let mut request = client.get(url);
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "无法连接模型目录，请检查接口地址或网络后重试")?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "模型目录鉴权失败（HTTP 401），请检查当前供应方的 API Key".into(),
            403 => "当前 Key 无权访问模型目录（HTTP 403），请检查账号权限或服务地区".into(),
            429 => "模型目录请求受额度或限流约束（HTTP 429），请稍后重试或检查供应方额度".into(),
            _ => format!(
                "模型目录接口返回 HTTP {}，请检查 Base URL 后重试",
                status.as_u16()
            ),
        });
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| "模型目录未返回有效 JSON，请检查 Base URL")?;
    let models = parse_models(&value, is_local);
    if models.is_empty() {
        return Err("当前 Key 的模型目录为空或格式不受支持，仍可手动填写官方模型 ID".into());
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn fixture(status: u16, body: Value) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut chunk = [0; 2048];
                let count = socket.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
            }
            let body = body.to_string();
            socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (format!("http://{address}/v1"), server)
    }

    #[tokio::test]
    async fn presets_use_only_key_scoped_models_endpoint_without_inference_or_tags_requests() {
        for provider in ["deepseek", "nvidia", "amd", "opencode"] {
            let (url, server) = fixture(200, json!({"data":[{"id":"DeepSeek-V4.1-Flash"}]})).await;
            let models = fetch_models_with_metadata(provider, &url, Some("fixture-provider-key"))
                .await
                .unwrap();
            assert_eq!(models[0].id, "DeepSeek-V4.1-Flash");
            let request = server.await.unwrap();
            assert!(request.starts_with("GET /v1/models HTTP/1.1"));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture-provider-key"));
            assert!(!request.contains("/api/tags"));
            assert!(!request.contains("/chat/completions"));
        }
    }

    #[tokio::test]
    async fn catalogue_failures_preserve_authentication_and_quota_meaning() {
        for (status, expected) in [(401, "鉴权失败"), (403, "无权访问"), (429, "额度或限流")]
        {
            let (url, server) = fixture(status, json!({"error":"fixture"})).await;
            let error = fetch_models_with_metadata("amd", &url, Some("fixture-key"))
                .await
                .unwrap_err();
            assert!(error.contains(expected));
            server.await.unwrap();
        }
        assert!(
            fetch_models_with_metadata("nvidia", "https://integrate.api.nvidia.com/v1", None)
                .await
                .unwrap_err()
                .contains("请先填写")
        );
    }

    #[test]
    fn cloud_catalogue_and_prices_are_not_free_quota_evidence() {
        let models = parse_models(
            &json!({"data":[{"id":"trial-model","pricing":{"prompt":"0","completion":"0"}},{"id":"trial-model"}]}),
            false,
        );
        assert_eq!(models.len(), 1);
        assert!(!models[0].is_free);
        assert_eq!(models[0].cost_input, None);
        assert!(parse_models(
            &json!({"models":[{"name":"public-cloud-catalogue"}]}),
            false
        )
        .is_empty());
    }
}
