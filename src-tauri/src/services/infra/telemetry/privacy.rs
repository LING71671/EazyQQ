//! Retain technical evidence and discard messages, payloads, identities and local paths.
use serde_json::Value;

fn clean_stack(stack: &mut Value) {
    if let Some(object) = stack.as_object_mut() { object.retain(|key, _| key == "frames"); }
    if let Some(frames) = stack.get_mut("frames").and_then(Value::as_array_mut) {
        for frame in frames {
            if let Some(frame) = frame.as_object_mut() {
                frame.retain(|key, _| ["filename", "function", "lineno", "colno", "in_app"].contains(&key.as_str()));
                if let Some(file) = frame.get_mut("filename") {
                    if let Some(name) = file.as_str() { *file = Value::from(name.rsplit(['/', '\\']).next().unwrap_or("source")); }
                }
            }
        }
    }
}

pub fn sanitize(value: Value) -> Option<Value> {
    let source = value.as_object()?;
    let mut clean = serde_json::Map::new();
    for key in ["event_id", "timestamp", "platform", "level", "release", "environment"] {
        if let Some(value) = source.get(key) { clean.insert(key.into(), value.clone()); }
    }
    clean.insert("message".into(), Value::from("EazyQQ runtime error; dynamic content remains in local diagnostics"));
    if let Some(stack) = source.get("stacktrace") { let mut stack = stack.clone(); clean_stack(&mut stack); clean.insert("stacktrace".into(), stack); }
    if let Some(threads) = source.get("threads").and_then(|threads| threads.get("values")).and_then(Value::as_array) {
        let values = threads.iter().map(|thread| {
            let mut item = serde_json::Map::new();
            for key in ["current", "crashed"] { if let Some(value) = thread.get(key) { item.insert(key.into(), value.clone()); } }
            if let Some(stack) = thread.get("stacktrace") { let mut stack = stack.clone(); clean_stack(&mut stack); item.insert("stacktrace".into(), stack); }
            Value::Object(item)
        }).collect::<Vec<_>>();
        clean.insert("threads".into(), serde_json::json!({"values":values}));
    }
    if let Some(trace) = source.get("contexts").and_then(|contexts| contexts.get("trace")).and_then(Value::as_object) {
        let trace = trace.iter().filter(|(key, _)| ["type", "trace_id", "span_id", "parent_span_id", "op", "status"].contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone())).collect::<serde_json::Map<_, _>>();
        clean.insert("contexts".into(), serde_json::json!({"trace":trace}));
    }
    if let Some(tags) = source.get("tags").and_then(Value::as_object) {
        let tags = tags.iter().filter(|(key, _)| ["component", "operation", "operation_id", "phase", "error_code", "provider"].contains(&key.as_str()))
            .filter(|(_, value)| value.as_str().is_some_and(|text| text.len() <= 128 && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))))
            .map(|(key, value)| (key.clone(), value.clone())).collect::<serde_json::Map<_, _>>();
        clean.insert("tags".into(), Value::Object(tags));
    }
    if let Some(exception) = source.get("exception") {
        let mut exception = exception.clone();
        if let Some(values) = exception.get_mut("values").and_then(Value::as_array_mut) {
            for value in values {
                if let Some(item) = value.as_object_mut() {
                    item.retain(|key, _| ["type", "stacktrace"].contains(&key.as_str()));
                    item.insert("value".into(), Value::from("Dynamic exception details withheld"));
                    if let Some(stack) = item.get_mut("stacktrace") { clean_stack(stack); }
                }
            }
        }
        clean.insert("exception".into(), exception);
    }
    Some(Value::Object(clean))
}

#[cfg(test)]
mod tests {
    #[test]
    fn event_drops_dynamic_data_but_retains_source_and_correlation() {
        let original = serde_json::json!({"message":"chat-private", "user":{"id":"10001"},"request":{"headers":{"Authorization":"private-key"}},"extra":{"token":"private-key"},"tags":{"component":"cli","operation_id":"aabb","uin":"10001"},"exception":{"values":[{"type":"Panic","value":"chat-private","stacktrace":{"frames":[{"filename":"C:\\Users\\private-user\\src\\main.rs","function":"main","lineno":12,"vars":{"key":"private-key"}}]}}]}});
        let clean = super::sanitize(original).unwrap();
        let serialized = clean.to_string();
        for private in ["chat-private", "private-key", "private-user", "10001"] { assert!(!serialized.contains(private)); }
        assert_eq!(clean["tags"]["component"], "cli");
        assert_eq!(clean["exception"]["values"][0]["stacktrace"]["frames"][0]["lineno"], 12);
    }
}
