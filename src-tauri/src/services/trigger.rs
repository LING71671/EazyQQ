//! Trigger evaluation for inbound messages.
//!
//! `contact_rules` has always carried `trigger_condition` and `keywords`, and the UI has
//! always let the user set them - but nothing ever read them. A group configured as
//! `at_me` + `auto_reply` would therefore answer *every* message. This module is the
//! missing enforcement.
//!
//! Conditions:
//! * `all`     - any message triggers
//! * `at_me`   - only when the account is @-mentioned (or @全体成员 is used)
//! * `keyword` - only when one of the configured keywords appears in the text

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerDecision {
    /// The message satisfies the configured condition.
    Matched,
    /// The message must be ignored, with the reason for the log line.
    Skipped(String),
}

impl TriggerDecision {
    pub fn is_matched(&self) -> bool {
        matches!(self, TriggerDecision::Matched)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            TriggerDecision::Skipped(r) => Some(r.as_str()),
            TriggerDecision::Matched => None,
        }
    }
}

/// Parse the JSON-encoded keyword list stored in `contact_rules.keywords`.
pub fn parse_keywords(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(|k| k.trim().to_lowercase())
        .filter(|k| !k.is_empty())
        .collect()
}

/// Case-insensitive substring match against any keyword.
pub fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let haystack = text.to_lowercase();
    keywords.iter().any(|k| haystack.contains(k.as_str()))
}

/// Was our account @-mentioned in this event?
///
/// Handles both the structured `message` array (OneBot 11) and the flattened
/// `[CQ:at,qq=...]` form in `raw_message`, since backends vary in what they send.
pub fn is_at_me(event: &Value, self_id: &str) -> bool {
    if self_id.is_empty() {
        return false;
    }

    if let Some(segments) = event.get("message").and_then(|m| m.as_array()) {
        for seg in segments {
            if seg.get("type").and_then(|t| t.as_str()) != Some("at") {
                continue;
            }
            let data = seg.get("data");
            let qq = data
                .and_then(|d| d.get("qq"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            // `all` means @全体成员, which also addresses us.
            if qq == self_id || qq == "all" {
                return true;
            }
        }
    }

    if let Some(raw) = event.get("raw_message").and_then(|r| r.as_str()) {
        if raw.contains(&format!("[CQ:at,qq={}]", self_id)) || raw.contains("[CQ:at,qq=all]") {
            return true;
        }
    }

    false
}

/// Decide whether this message should be processed under the configured condition.
pub fn evaluate(
    condition: &str,
    keywords: &[String],
    event: &Value,
    self_id: &str,
    text: &str,
) -> TriggerDecision {
    match condition {
        // Unknown conditions fail closed rather than defaulting to "answer everything".
        "" | "all" => TriggerDecision::Matched,
        "at_me" => {
            if is_at_me(event, self_id) {
                TriggerDecision::Matched
            } else {
                TriggerDecision::Skipped("未 @ 到本账号".to_string())
            }
        }
        "keyword" => {
            if keywords.is_empty() {
                TriggerDecision::Skipped("触发条件为关键词但未配置任何关键词".to_string())
            } else if matches_keywords(text, keywords) {
                TriggerDecision::Matched
            } else {
                TriggerDecision::Skipped(format!("未命中关键词 ({})", keywords.join("/")))
            }
        }
        other => TriggerDecision::Skipped(format!("未知触发条件 {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SELF: &str = "462564834";

    fn at_event(qq: &str) -> Value {
        json!({
            "message": [
                { "type": "at", "data": { "qq": qq } },
                { "type": "text", "data": { "text": " 帮我看下" } }
            ],
            "raw_message": format!("[CQ:at,qq={}] 帮我看下", qq)
        })
    }

    #[test]
    fn all_condition_always_matches() {
        let plain = json!({ "raw_message": "随便说点什么" });
        assert!(evaluate("all", &[], &plain, SELF, "随便说点什么").is_matched());
        // An empty condition must behave like `all`, not like "deny".
        assert!(evaluate("", &[], &plain, SELF, "随便说点什么").is_matched());
    }

    #[test]
    fn at_me_detects_structured_segments() {
        assert!(evaluate("at_me", &[], &at_event(SELF), SELF, "帮我看下").is_matched());
    }

    #[test]
    fn at_me_detects_the_cq_form_in_raw_message() {
        let event = json!({ "raw_message": format!("[CQ:at,qq={}] 在吗", SELF) });
        assert!(evaluate("at_me", &[], &event, SELF, "在吗").is_matched());
    }

    #[test]
    fn at_me_treats_at_all_as_addressing_us() {
        assert!(evaluate("at_me", &[], &at_event("all"), SELF, "通知").is_matched());
    }

    #[test]
    fn at_me_skips_when_someone_else_is_mentioned() {
        let decision = evaluate("at_me", &[], &at_event("10001"), SELF, "帮我看下");
        assert!(!decision.is_matched());
        assert!(decision.reason().unwrap().contains("未 @"));
    }

    #[test]
    fn at_me_skips_a_plain_message() {
        let event = json!({ "raw_message": "大家早" });
        assert!(!evaluate("at_me", &[], &event, SELF, "大家早").is_matched());
    }

    #[test]
    fn at_me_without_a_self_id_fails_closed() {
        // Better to stay silent than to answer every message in the group.
        assert!(!evaluate("at_me", &[], &at_event(SELF), "", "hi").is_matched());
    }

    #[test]
    fn keyword_condition_matches_case_insensitively() {
        let keywords = parse_keywords(r#"["EazyQQ","进度"]"#);
        let event = json!({ "raw_message": "eazyqq 现在什么进度" });
        assert!(evaluate("keyword", &keywords, &event, SELF, "eazyqq 现在什么进度").is_matched());
    }

    #[test]
    fn keyword_condition_skips_when_nothing_matches() {
        let keywords = parse_keywords(r#"["进度"]"#);
        let decision = evaluate("keyword", &keywords, &json!({}), SELF, "今天天气不错");
        assert!(!decision.is_matched());
        assert!(decision.reason().unwrap().contains("未命中关键词"));
    }

    #[test]
    fn keyword_condition_with_no_keywords_is_skipped_not_answered() {
        let decision = evaluate("keyword", &[], &json!({}), SELF, "任意内容");
        assert!(!decision.is_matched());
        assert!(decision.reason().unwrap().contains("未配置"));
    }

    #[test]
    fn unknown_condition_fails_closed() {
        let decision = evaluate("whatever", &[], &json!({}), SELF, "hi");
        assert!(!decision.is_matched());
        assert!(decision.reason().unwrap().contains("未知触发条件"));
    }

    #[test]
    fn keyword_parsing_ignores_blanks_and_normalises_case() {
        let keywords = parse_keywords(r#"[" EazyQQ ", "", "  ", "进度"]"#);
        assert_eq!(keywords, vec!["eazyqq", "进度"]);
        assert!(parse_keywords("not json").is_empty());
    }
}
