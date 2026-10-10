use std::collections::HashMap;

pub struct Args {
    pub command: String,
    pub flags: HashMap<String, String>,
    pub switches: Vec<String>,
    /// Bare positional arguments after the command, reserved for future use
    /// (e.g. `eazyqq-cli rule 1104661022 --mode copilot`).
    #[allow(dead_code)]
    pub positional: Vec<String>,
}

impl Args {
    pub fn parse() -> Self {
        Self::parse_from(std::env::args().skip(1))
    }

    pub fn parse_from(raw: impl IntoIterator<Item = String>) -> Self {
        let raw: Vec<String> = raw.into_iter().collect();
        let mut command = String::new();
        let mut flags = HashMap::new();
        let mut switches = Vec::new();
        let mut positional = Vec::new();

        let mut i = 0;
        while i < raw.len() {
            let token = &raw[i];
            if let Some(name) = token.strip_prefix("--") {
                // `--key=value` form
                if let Some((k, v)) = name.split_once('=') {
                    flags.insert(k.to_string(), v.to_string());
                } else if matches!(
                    name,
                    "json"
                        | "probe"
                        | "all"
                        | "help"
                        | "deep"
                        | "browser"
                        | "whitelist"
                        | "summary-only"
                        | "dry-run"
                        | "confirm"
                        | "stream"
                        | "refresh"
                ) {
                    switches.push(name.to_string());
                } else if i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
                    flags.insert(name.to_string(), raw[i + 1].clone());
                    i += 1;
                } else {
                    switches.push(name.to_string());
                }
            } else if command.is_empty() {
                command = token.clone();
            } else {
                positional.push(token.clone());
            }
            i += 1;
        }

        Args {
            command,
            flags,
            switches,
            positional,
        }
    }

    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(|s| s.as_str())
    }

    pub fn has(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name) || self.flags.contains_key(name)
    }

    pub fn json(&self) -> bool {
        self.has("json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boolean_flags_do_not_consume_commands_or_subcommands() {
        let args = Args::parse_from(["--json", "instances", "list"].map(str::to_string));
        assert_eq!(args.command, "instances");
        assert_eq!(args.positional, ["list"]);
        assert!(args.json());
        let args = Args::parse_from(["--account=10001", "status", "--json"].map(str::to_string));
        assert_eq!(args.flag("account"), Some("10001"));
        assert_eq!(args.command, "status");
    }
}
