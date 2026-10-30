use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct NativeFace {
    pub id: String,
    pub name: String,
}

pub fn catalog() -> &'static [NativeFace] {
    static CATALOG: OnceLock<Vec<NativeFace>> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/../src/data/qq-faces.json"
    ))).expect("bundled QQ face catalog must be valid"))
}

pub fn face_name(id: &str) -> Option<&str> {
    catalog().iter().find(|face| face.id == id).map(|face| face.name.as_str())
}

pub fn prompt_catalog() -> String {
    let entries = catalog().iter().map(|face| format!("{}={}", face.id, face.name)).collect::<Vec<_>>().join("，");
    format!("可选QQ原生表情目录：{entries}。需要时用 [CQ:face,id=编号] 表达；可以只发一个表情，或与文字在同一条消息混排，也可以完全不用。不要用Unicode emoji，不生成目录外编号、图片码、at或其他CQ码。表情不是每次回复的固定后缀。")
}

fn unescape_text(text: &str) -> String {
    text.replace("&#91;", "[").replace("&#93;", "]").replace("&#44;", ",").replace("&amp;", "&")
}

pub fn segments(content: &str) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("[CQ:") {
        if start > 0 {
            result.push(json!({"type":"text", "data":{"text":unescape_text(&remaining[..start])}}));
        }
        let code = &remaining[start..];
        let end = code.find(']').ok_or("QQ表情码不完整，请重新生成回复")?;
        let id = code[..end].strip_prefix("[CQ:face,id=")
            .filter(|id| face_name(id).is_some())
            .ok_or("回复含未支持的QQ表情编号或消息段，请重新生成回复")?;
        result.push(json!({"type":"face", "data":{"id":id}}));
        remaining = &code[end + 1..];
    }
    if !remaining.is_empty() {
        result.push(json!({"type":"text", "data":{"text":unescape_text(remaining)}}));
    }
    Ok(result)
}

fn contains_unicode_emoji(text: &str) -> bool {
    text.chars().any(|ch| matches!(ch as u32,
        0x1F000..=0x1FAFF | 0x20E3 | 0xFE0F |
        0x2194..=0x2199 | 0x21A9..=0x21AA | 0x231A..=0x231B | 0x2328 | 0x23CF |
        0x23E9..=0x23F3 | 0x23F8..=0x23FA | 0x24C2 |
        0x25AA..=0x25AB | 0x25B6 | 0x25C0 | 0x25FB..=0x25FE |
        0x2600..=0x27BF | 0x2934..=0x2935 | 0x2B05..=0x2B07 |
        0x2B1B..=0x2B1C | 0x2B50 | 0x2B55 | 0x3030 | 0x303D | 0x3297 | 0x3299
    ))
}

pub fn normalize_reply(content: &str) -> Result<String, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("大模型返回了空回复".into());
    }
    if contains_unicode_emoji(content) {
        return Err("生成的回复含Unicode emoji，请重新生成或改用QQ原生表情".into());
    }
    segments(content)?;
    Ok(content.to_string())
}

pub fn outgoing_message(content: &str) -> Result<Option<Value>, String> {
    if content.contains("[CQ:face") {
        return segments(content).map(|segments| Some(Value::Array(segments)));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_pinned_protocol_coverage_with_exact_unique_ids() {
        let faces=catalog();
        assert_eq!(faces.len(),407);
        for pair in faces.windows(2) { assert!(pair[0].id.parse::<u32>().unwrap()<pair[1].id.parse::<u32>().unwrap()); }
        for (id,name) in [("264","捂脸"),("277","汪汪"),("317","菜汪"),("324","吃糖"),("338","我想开了"),("427","偷感")] {
            assert_eq!(face_name(id),Some(name));
        }
        assert_eq!(face_name("999999"),None);
    }

    #[test]
    fn standalone_face_is_one_native_segment_without_added_text() {
        assert_eq!(outgoing_message("[CQ:face,id=76]").unwrap(), Some(json!([
            {"type":"face","data":{"id":"76"}}
        ])));
        assert_eq!(normalize_reply("[CQ:face,id=76]").unwrap(), "[CQ:face,id=76]");
    }

    #[test]
    fn current_animated_faces_remain_native_face_segments() {
        assert_eq!(outgoing_message("[CQ:face,id=427]").unwrap(),Some(json!([{"type":"face","data":{"id":"427"}}])));
        assert_eq!(outgoing_message("这下明白了[CQ:face,id=324][CQ:face,id=427]").unwrap(),Some(json!([
            {"type":"text","data":{"text":"这下明白了"}},
            {"type":"face","data":{"id":"324"}},
            {"type":"face","data":{"id":"427"}}
        ])));
    }

    #[tokio::test]
    async fn bound_onebot_sends_current_faces_as_native_segments_not_images() {
        use std::sync::{Arc,Mutex};
        use tokio::io::{AsyncReadExt,AsyncWriteExt};
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint=format!("http://{}",listener.local_addr().unwrap());
        let sent=Arc::new(Mutex::new(Vec::<Value>::new()));let captured=sent.clone();
        let task=tokio::spawn(async move {
            while let Ok((mut socket,_))=listener.accept().await {
                let mut bytes=Vec::new();let header_end;
                loop {
                    let mut chunk=[0u8;4096];let count=socket.read(&mut chunk).await.unwrap();assert!(count>0);bytes.extend_from_slice(&chunk[..count]);assert!(bytes.len()<16384);
                    if let Some(end)=bytes.windows(4).position(|window|window==b"\r\n\r\n") {
                        let headers=String::from_utf8_lossy(&bytes[..end]);
                        let length=headers.lines().find_map(|line|line.split_once(':').filter(|(key,_)|key.eq_ignore_ascii_case("content-length")).and_then(|(_,value)|value.trim().parse::<usize>().ok())).unwrap_or(0);
                        if bytes.len()>=end+4+length {header_end=end+4;break;}
                    }
                }
                let request=String::from_utf8_lossy(&bytes[..header_end]);let path=request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                let data=if path=="/get_login_info" {json!({"user_id":10001})} else {assert_eq!(path,"/send_msg");captured.lock().unwrap().push(serde_json::from_slice(&bytes[header_end..]).unwrap());json!({"message_id":17})};
                let body=json!({"status":"ok","retcode":0,"data":data}).to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        });
        let client=crate::services::protocol::OneBotClient::for_account(endpoint,Some("10001".into()));
        client.send_msg("group","995001","[CQ:face,id=427]").await.unwrap();
        client.send_msg("group","995001","收到[CQ:face,id=324][CQ:face,id=427]").await.unwrap();
        let messages=sent.lock().unwrap();assert_eq!(messages.len(),2);
        assert_eq!(messages[0]["message"],json!([{"type":"face","data":{"id":"427"}}]));
        assert_eq!(messages[1]["message"],json!([{"type":"text","data":{"text":"收到"}},{"type":"face","data":{"id":"324"}},{"type":"face","data":{"id":"427"}}]));
        assert!(messages.iter().all(|body|body["message"].as_array().unwrap().iter().all(|segment|segment["type"]!="image")));task.abort();
    }

    #[test]
    fn mixed_face_and_text_keep_one_message_and_original_order() {
        assert_eq!(segments("收到[CQ:face,id=124]明天说").unwrap(), vec![
            json!({"type":"text","data":{"text":"收到"}}),
            json!({"type":"face","data":{"id":"124"}}),
            json!({"type":"text","data":{"text":"明天说"}})
        ]);
        assert_eq!(outgoing_message("普通文字").unwrap(), None);
    }

    #[test]
    fn unknown_faces_or_injected_protocol_actions_are_rejected() {
        for content in ["[CQ:face,id=999999]", "[CQ:at,qq=all]", "[CQ:face,id=76,foo=bar]", "[CQ:face,id=76"] {
            assert!(normalize_reply(content).is_err(), "{content}");
        }
    }

    #[test]
    fn unicode_output_is_rejected_without_silently_editing_it() {
        for text in ["好\u{1f602}", "\u{1f1e8}\u{1f1f3}", "1\u{fe0f}\u{20e3}", "\u{1f469}\u{200d}\u{1f4bb}",
            "\u{263a}", "\u{2764}", "\u{270c}", "\u{2600}", "\u{2197}", "\u{00a9}\u{fe0f}"] {
            assert!(normalize_reply(text).is_err());
        }
        assert_eq!(normalize_reply("x² + y² = 25，约等于5。好").unwrap(), "x² + y² = 25，约等于5。好");
    }
}
