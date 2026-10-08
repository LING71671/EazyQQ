use crate::args::Args;
use crate::format::print_json;
use serde_json::json;

pub const COMMANDS: &[&str] = &[
    "accounts",
    "ai-config",
    "ai-detect",
    "ai-models",
    "ai-set",
    "ai-test",
    "ask",
    "batch-mode",
    "chain-status",
    "config",
    "config-audit",
    "contacts",
    "draft-dismiss",
    "draft-regenerate",
    "draft-send",
    "drafts",
    "export",
    "file-download",
    "file-summarize",
    "files",
    "folder",
    "friends",
    "groups",
    "health",
    "help",
    "history",
    "instances",
    "log-path",
    "log-tail",
    "login-info",
    "logout",
    "mark-read",
    "mcp",
    "napcat-doctor",
    "qq-path",
    "qr",
    "quick-login",
    "quick-login-list",
    "repair",
    "restart",
    "run",
    "rule",
    "scheduler-tick",
    "schema",
    "selftest",
    "send",
    "set-config",
    "simulate",
    "start",
    "status",
    "stop",
    "summaries",
    "summarize",
    "summary-delete",
    "updates",
    "version",
    "whitelist-groups",
    "window",
];

pub fn cmd_schema(_args: &Args) -> Result<(), String> {
    let mut schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../../docs/api/cli.schema.json"))
            .map_err(|e| e.to_string())?;
    schema["version"] = json!(env!("CARGO_PKG_VERSION"));
    print_json(&schema);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_covers_every_dispatch_command() {
        let source = include_str!("../../main.rs");
        for line in source.lines().filter(|line| line.contains(" => ")) {
            if let Some(name) = line
                .trim()
                .strip_prefix('"')
                .and_then(|line| line.split('"').next())
            {
                assert!(COMMANDS.contains(&name), "missing {}", name);
            }
        }
        assert_eq!(
            COMMANDS.len(),
            COMMANDS
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
    }
}
