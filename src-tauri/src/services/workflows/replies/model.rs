use crate::models::MessageItemDto;
use crate::services::protocol::native_face;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_BUBBLES: usize = 6;
const MAX_SEGMENTS: usize = 32;
const MAX_CHARS: usize = 2_000;
const MAX_BYTES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReplySegment {
    Text {
        text: String,
    },
    Face {
        #[serde(rename = "faceKey")]
        face_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplyBubble {
    #[serde(default)]
    pub id: String,
    pub segments: Vec<ReplySegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplyPlan {
    pub schema_version: u32,
    pub bubbles: Vec<ReplyBubble>,
}

impl ReplyBubble {
    pub fn content(&self) -> String {
        self.segments
            .iter()
            .map(|segment| match segment {
                ReplySegment::Text { text } => text.clone(),
                ReplySegment::Face { face_key } => format!("[CQ:face,id={face_key}]"),
            })
            .collect()
    }
}

impl ReplyPlan {
    pub fn from_bubble_texts(contents: &[String]) -> Result<Self, String> {
        let mut bubbles = Vec::with_capacity(contents.len().min(MAX_BUBBLES));
        if contents.len() > MAX_BUBBLES {
            return Err("回复计划最多包含6条消息".into());
        }
        for (index, content) in contents.iter().enumerate() {
            let normalized = native_face::normalize_reply(content)?;
            let segments = native_face::segments(&normalized)?
                .iter()
                .map(|segment| {
                    if segment["type"] == "face" {
                        ReplySegment::Face {
                            face_key: segment["data"]["id"].as_str().unwrap_or_default().into(),
                        }
                    } else {
                        ReplySegment::Text {
                            text: segment["data"]["text"].as_str().unwrap_or_default().into(),
                        }
                    }
                })
                .collect();
            bubbles.push(ReplyBubble {
                id: format!("b{}", index + 1),
                segments,
            });
        }
        let plan = Self {
            schema_version: 1,
            bubbles,
        };
        plan.validate()?;
        Ok(plan)
    }

    pub fn parse_candidate(content: &str) -> Result<Self, String> {
        let content = content.trim();
        if content.len() > MAX_BYTES {
            return Err("回复计划超过32 KiB限制".into());
        }
        let json = content
            .strip_prefix("```json")
            .or_else(|| content.strip_prefix("```"))
            .and_then(|text| text.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(content);
        let mut plan =
            if json.starts_with('{') || (json.starts_with('[') && !json.starts_with("[CQ:face,")) {
                serde_json::from_str::<Self>(json)
                    .map_err(|error| format!("回复计划格式无效: {error}"))?
            } else {
                let normalized = native_face::normalize_reply(content)?;
                let segments = native_face::segments(&normalized)?
                    .iter()
                    .map(|segment| {
                        if segment["type"] == "face" {
                            ReplySegment::Face {
                                face_key: segment["data"]["id"].as_str().unwrap_or_default().into(),
                            }
                        } else {
                            ReplySegment::Text {
                                text: segment["data"]["text"].as_str().unwrap_or_default().into(),
                            }
                        }
                    })
                    .collect();
                Self {
                    schema_version: 1,
                    bubbles: vec![ReplyBubble {
                        id: "b1".into(),
                        segments,
                    }],
                }
            };
        // Model-provided identifiers never determine persistent receipt identities.
        for (index, bubble) in plan.bubbles.iter_mut().enumerate() {
            bubble.id = format!("b{}", index + 1);
        }
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("不支持的回复计划版本".into());
        }
        if self.bubbles.is_empty() || self.bubbles.len() > MAX_BUBBLES {
            return Err("回复计划须包含1至6条消息".into());
        }
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("回复计划超过32 KiB限制".into());
        }
        let mut ids = std::collections::HashSet::new();
        for bubble in &self.bubbles {
            if bubble.id.is_empty()
                || bubble.id.len() > 64
                || !bubble
                    .id
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'-')
                || !ids.insert(&bubble.id)
            {
                return Err("回复消息编号无效或重复".into());
            }
            if bubble.segments.is_empty() || bubble.segments.len() > MAX_SEGMENTS {
                return Err("每条消息须包含1至32个文字或表情段".into());
            }
            let mut visible = false;
            for segment in &bubble.segments {
                match segment {
                    ReplySegment::Text { text } => {
                        if text.contains("[CQ:") {
                            return Err("文字段不能嵌入协议码，请使用表情段".into());
                        }
                        if text
                            .chars()
                            .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
                        {
                            return Err("回复文字包含无效控制字符".into());
                        }
                        if !text.trim().is_empty() {
                            native_face::normalize_reply(text)?;
                            visible = true;
                        }
                    }
                    ReplySegment::Face { face_key } => {
                        if native_face::face_name(face_key).is_none() {
                            return Err("回复包含目录外的QQ原生表情".into());
                        }
                        visible = true;
                    }
                }
            }
            if !visible || bubble.content().chars().count() > MAX_CHARS {
                return Err("每条回复不能为空且不能超过2000字".into());
            }
        }
        Ok(())
    }

    pub fn preview(&self) -> String {
        self.bubbles
            .iter()
            .map(ReplyBubble::content)
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub(crate) fn fingerprint(&self) -> Result<String, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn prompt_schema() -> &'static str {
        "仅返回JSON回复计划：{\"schemaVersion\":1,\"bubbles\":[{\"segments\":[{\"type\":\"text\",\"text\":\"一句自然的话\"}]},{\"segments\":[{\"type\":\"face\",\"faceKey\":\"324\"}]}]}。按对话需要选择1至6条短消息；示例不是固定模板。每条可仅文字、仅QQ原生表情或二者混排。一个bubble对应一次发送；文字中的换行不会拆成多条。每条最多2000字、32段。只允许text和face段，不输出目标、账号、任务编号或其他字段。不要每次都追问或加表情。"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleReceipt {
    pub bubble_id: String,
    pub status: String,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryReport {
    pub batch_id: String,
    pub status: String,
    pub receipts: Vec<BubbleReceipt>,
    #[serde(default)]
    pub messages: Vec<MessageItemDto>,
}
