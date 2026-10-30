//! Account-local expression overrides. These never grant identity or tool permissions.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::db::Database;
use crate::services::protocol::context::ConversationScope;

const SETTING: &str = "conversation_personas_v1";

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonaOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_uin: Option<String>,
    pub text: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersonaSettings {
    revision: u64,
    overrides: Vec<PersonaOverride>,
}

pub struct ResolvedPersona {
    pub text: String,
    pub revision: u64,
}

fn read(db: &Database) -> Result<PersonaSettings, String> {
    db.get_setting(SETTING)
        .map_err(|error| error.to_string())?
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| "人设配置损坏，未采用未知内容".into())
        })
        .unwrap_or_else(|| Ok(PersonaSettings::default()))
}

fn matches(left: &PersonaOverride, right: &PersonaOverride) -> bool {
    left.target_id == right.target_id
        && left.target_type == right.target_type
        && left.member_uin == right.member_uin
}

fn valid_id(value: &str) -> bool {
    (5..=20).contains(&value.len())
        && !value.starts_with('0')
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn selector(request: &Value) -> Result<PersonaOverride, String> {
    let target_id = request
        .get("targetId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let target_type = request
        .get("targetType")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let member_uin = request
        .get("memberUin")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if target_id.as_deref().is_some_and(|id| !valid_id(id))
        || member_uin.as_deref().is_some_and(|id| !valid_id(id))
    {
        return Err("人设目标必须是有效的 QQ 号或群号".into());
    }
    if target_id.is_some() != target_type.is_some()
        || target_type
            .as_deref()
            .is_some_and(|kind| !matches!(kind, "private" | "group"))
        || (member_uin.is_some() && target_type.as_deref() != Some("group"))
    {
        return Err("人设作用域无效；群成员覆盖必须绑定具体群".into());
    }
    Ok(PersonaOverride {
        target_id,
        target_type,
        member_uin,
        text: String::new(),
    })
}

fn resolve_settings(
    settings: &PersonaSettings,
    selection: &PersonaOverride,
    include_selected: bool,
) -> String {
    let mut layers: Vec<&PersonaOverride> = settings
        .overrides
        .iter()
        .filter(|item| {
            let account = item.target_id.is_none();
            let target =
                item.target_id == selection.target_id && item.target_type == selection.target_type;
            let applicable = account
                || (target
                    && (item.member_uin.is_none() || item.member_uin == selection.member_uin));
            applicable && (include_selected || !matches(item, selection))
        })
        .collect();
    layers.sort_by_key(|item| (item.target_id.is_some(), item.member_uin.is_some()));
    layers
        .iter()
        .map(|item| {
            let label = if item.member_uin.is_some() {
                "本群此成员的表达偏好"
            } else if item.target_id.is_some() {
                "当前会话的表达偏好"
            } else {
                "当前账号的表达偏好"
            };
            format!("{label}：\n{}", item.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn resolve(
    db: &Database,
    scope: &ConversationScope,
    member_uin: Option<&str>,
) -> Result<ResolvedPersona, String> {
    let selection = PersonaOverride {
        target_id: Some(scope.target_id.clone()),
        target_type: Some(scope.target_type.clone()),
        member_uin: member_uin.map(str::to_owned),
        text: String::new(),
    };
    let settings = read(db)?;
    Ok(ResolvedPersona {
        text: resolve_settings(&settings, &selection, true),
        revision: settings.revision,
    })
}

pub fn manage(db: &Database, request: Value) -> Result<Value, String> {
    if !request.is_object() {
        return Err("人设请求必须是对象".into());
    }
    let mut selection = selector(&request)?;
    match request["operation"].as_str().unwrap_or("get") {
        "get" => {}
        operation @ ("set" | "clear") => {
            selection.text = request["text"].as_str().unwrap_or("").trim().to_owned();
            if selection.text.chars().count() > 4096 {
                return Err("人设最多 4096 字".into());
            }
            if operation == "set" && selection.text.is_empty() {
                return Err("人设内容不能为空；恢复继承请使用清除".into());
            }
            db.with_connection(|connection| {
                let transaction = connection
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let value = crate::services::storage::settings::get_setting(&transaction, SETTING)?;
                let mut settings: PersonaSettings = value
                    .map(|value| {
                        serde_json::from_str(&value).map_err(|_| rusqlite::Error::InvalidQuery)
                    })
                    .transpose()?
                    .unwrap_or_default();
                settings.overrides.retain(|item| !matches(item, &selection));
                if operation == "set" {
                    if settings.overrides.len() >= 256 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    settings.overrides.push(selection.clone());
                }
                settings.revision += 1;
                crate::services::storage::settings::set_setting(
                    &transaction,
                    SETTING,
                    &serde_json::to_string(&settings).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    0,
                )?;
                transaction.commit()
            })
            .map_err(|error| format!("保存人设失败: {error}"))?;
        }
        _ => return Err("未知人设操作".into()),
    }
    let settings = read(db)?;
    let text = settings
        .overrides
        .iter()
        .find(|item| matches(item, &selection))
        .map(|item| item.text.clone())
        .unwrap_or_default();
    Ok(
        json!({"text":text,"inheritedText":resolve_settings(&settings,&selection,false),"revision":settings.revision,"overrides":settings.overrides}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_are_inherited_by_scope_and_can_be_cleared() {
        let db = Database::init(
            crate::services::logging::workspace_root().join("persona-scopes/eazyqq.db"),
        )
        .unwrap();
        manage(&db, json!({"operation":"set","text":"account style"})).unwrap();
        manage(
            &db,
            json!({"operation":"set","targetType":"group","targetId":"20001","text":"group style"}),
        )
        .unwrap();
        manage(&db,json!({"operation":"set","targetType":"group","targetId":"20001","memberUin":"30001","text":"member style"})).unwrap();
        let scope = ConversationScope {
            account_uin: "10001".into(),
            target_type: "group".into(),
            target_id: "20001".into(),
        };
        let prompt = resolve(&db, &scope, Some("30001")).unwrap().text;
        assert!(
            prompt.contains("account style")
                && prompt.contains("group style")
                && prompt.contains("member style")
        );
        assert!(!resolve(&db, &scope, Some("30002"))
            .unwrap()
            .text
            .contains("member style"));
        let private = ConversationScope {
            target_type: "private".into(),
            ..scope.clone()
        };
        assert!(!resolve(&db, &private, None)
            .unwrap()
            .text
            .contains("group style"));
        manage(&db,json!({"operation":"clear","targetType":"group","targetId":"20001","memberUin":"30001"})).unwrap();
        assert!(!resolve(&db, &scope, Some("30001"))
            .unwrap()
            .text
            .contains("member style"));
        assert!(manage(&db,json!({"operation":"set","targetType":"private","targetId":"20001","memberUin":"30001","text":"bad"})).is_err());
    }
}
