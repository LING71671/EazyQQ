//! Native OpenCode transport. Free-tier models require the OpenCode runtime.
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub fn binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("EAZYQQ_OPENCODE_BIN")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(path);
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH")?) {
        for rel in [
            "opencode.exe",
            "node_modules/opencode-ai/bin/opencode.exe",
            "opencode",
        ] {
            let path = dir.join(rel);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn command() -> Result<tokio::process::Command, String> {
    let binary = binary().ok_or("OpenCode runtime was not found on PATH. Install OpenCode or set EAZYQQ_OPENCODE_BIN to its native executable")?;
    let mut command = tokio::process::Command::new(binary);
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    Ok(command)
}

pub fn model_id(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("opencode/{}", model)
    }
}

pub fn event_text(event: &Value) -> Option<&str> {
    if event["type"].as_str() == Some("text") {
        event["part"]["text"].as_str()
    } else {
        None
    }
}

pub async fn complete<F>(
    runtime_dir: &std::path::Path,
    model: &str,
    api_key: &str,
    payload: &Value,
    mut on_chunk: F,
) -> Result<(String, Option<String>), String>
where
    F: FnMut(&str) + Send,
{
    if model.trim().is_empty() {
        return Err("Select an available OpenCode model".into());
    }
    let cwd = runtime_dir;
    std::fs::create_dir_all(&cwd).map_err(|e| e.to_string())?;
    let mut config = serde_json::json!({
        "$schema":"https://opencode.ai/config.json",
        "permission":{"*":"ask","bash":"ask","edit":"ask","external_directory":"deny","task":"ask","skill":"ask","webfetch":"ask","websearch":"ask","question":"deny"},
        "agent":{"build":{"permission":{"*":"ask","external_directory":"deny","question":"deny"}}}
    });
    if !api_key.trim().is_empty() {
        config["provider"] = serde_json::json!({"opencode":{"options":{"apiKey":api_key}}});
    }
    let mut command = command()?;
    command
        .args([
            "run",
            "--pure",
            "--format",
            "json",
            "--model",
            &model_id(model),
            "--agent",
            "build",
        ])
        .current_dir(cwd)
        .env("OPENCODE_CONFIG_CONTENT", config.to_string())
        .env("XDG_DATA_HOME", runtime_dir.join("data"))
        .env("XDG_STATE_HOME", runtime_dir.join("state"))
        .env("OPENCODE_DISABLE_DEFAULT_PLUGINS", "true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start OpenCode: {}", e))?;
    let mut stdin = child.stdin.take().ok_or("OpenCode stdin unavailable")?;
    let input = format!("Respond to this conversation. Preserve its role hierarchy. Output only the assistant response.\n{}", payload["messages"]);
    stdin
        .write_all(input.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    drop(stdin);
    let output = child.stdout.take().ok_or("OpenCode stdout unavailable")?;
    let mut lines = BufReader::new(output).lines();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_requested = false;
    loop {
        let line = tokio::time::timeout_at(deadline, lines.next_line())
            .await
            .map_err(|_| "OpenCode inference timed out after 120 seconds")?
            .map_err(|e| e.to_string())?;
        let Some(line) = line else {
            break;
        };
        if line.len() > 2_000_000 || text.len() + reasoning.len() > 2_000_000 {
            return Err("OpenCode response exceeded 2 MB".into());
        }
        if let Ok(event) = serde_json::from_str::<Value>(&line) {
            if event["type"].as_str() == Some("error") {
                return Err(format!(
                    "OpenCode inference failed: {}",
                    crate::services::diagnostics::redact(
                        event["error"]["data"]["message"]
                            .as_str()
                            .unwrap_or("OpenCode reported an inference error"),
                        &[]
                    )
                ));
            }
            if event["type"].as_str() == Some("tool_use") {
                tool_requested = true;
            }
            if let Some(chunk) = event_text(&event) {
                text.push_str(chunk);
                on_chunk(chunk);
            }
            if event["type"].as_str() == Some("reasoning") {
                if let Some(chunk) = event["part"]["text"].as_str() {
                    reasoning.push_str(chunk);
                }
            }
        }
    }
    let status = tokio::time::timeout_at(deadline, child.wait())
        .await
        .map_err(|_| "OpenCode did not exit")?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("OpenCode exited with {}", status));
    }
    if text.trim().is_empty() && tool_requested {
        return Err(
            "OpenCode requested a tool operation. Noninteractive tool authorization was denied"
                .into(),
        );
    }
    if text.trim().is_empty() {
        return Err(
            "OpenCode returned no assistant text; check the selected model in OpenCode".into(),
        );
    }
    Ok((
        text,
        if reasoning.is_empty() {
            None
        } else {
            Some(reasoning)
        },
    ))
}

pub async fn models() -> Result<Vec<String>, String> {
    let mut command = command()?;
    command.args(["models", "opencode"]).stderr(Stdio::null());
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), command.output())
        .await
        .map_err(|_| "OpenCode model discovery timed out")?
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("OpenCode model discovery failed".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().strip_prefix("opencode/").map(str::to_string))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_only_assistant_text_events() {
        assert_eq!(
            event_text(&serde_json::json!({"type":"text","part":{"text":"hello"}})),
            Some("hello")
        );
        assert_eq!(
            event_text(&serde_json::json!({"type":"tool_use","part":{"text":"unsafe"}})),
            None
        );
        assert_eq!(model_id("big-pickle"), "opencode/big-pickle");
    }
}
