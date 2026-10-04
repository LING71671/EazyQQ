use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerDecision {
    Matched,
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

pub fn parse_keywords(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(|k| k.trim().to_lowercase())
        .filter(|k| !k.is_empty())
        .collect()
}

pub fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let haystack = text.to_lowercase();
    keywords.iter().any(|k| haystack.contains(k.as_str()))
}

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

pub fn evaluate(
    condition: &str,
    keywords: &[String],
    event: &Value,
    self_id: &str,
    text: &str,
) -> TriggerDecision {
    match condition {
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
