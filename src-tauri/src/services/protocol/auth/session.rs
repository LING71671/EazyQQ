//! Shared, read-only login evidence for the GUI, CLI and health monitor.
use super::{NapCatService, OneBotClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginEvidence {
    pub logged_in: bool,
    pub uin: Option<String>,
    pub nickname: Option<String>,
    pub source: String,
}

pub fn account_id(value: &Value) -> Option<String> {
    let id = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    validate_uin(&id).ok().map(|_| id)
}

pub use crate::services::identity::machine::validate_uin;

pub fn from_onebot(value: &Value) -> LoginEvidence {
    if value.get("status").and_then(Value::as_str) != Some("ok")
        || value.get("retcode").and_then(Value::as_i64) != Some(0)
    {
        return LoginEvidence::default();
    }
    let data = &value["data"];
    let uin = account_id(&data["user_id"]);
    LoginEvidence {
        logged_in: uin.is_some(),
        uin,
        nickname: data["nickname"].as_str().map(str::to_string),
        source: "onebot".into(),
    }
}

pub fn from_webui(value: &Value) -> LoginEvidence {
    let data = &value["data"];
    let logged_in = value["code"].as_i64() == Some(0)
        && (data["isLogin"].as_bool() == Some(true) || data["isLoggedIn"].as_bool() == Some(true));
    LoginEvidence {
        logged_in,
        uin: if logged_in {
            account_id(&data["uin"]).or_else(|| account_id(&data["user_id"]))
        } else {
            None
        },
        nickname: data["nick"]
            .as_str()
            .or_else(|| data["nickname"].as_str())
            .map(str::to_string),
        source: "webui".into(),
    }
}

pub async fn probe(napcat: &NapCatService, onebot: &OneBotClient) -> LoginEvidence {
    if let Ok(info) = onebot.get_login_info().await {
        let evidence = from_onebot(&info);
        if evidence.logged_in {
            return evidence;
        }
    }
    if let Ok(info) = napcat.check_login().await {
        let mut evidence = from_webui(&info);
        if evidence.logged_in && evidence.uin.is_none() {
            if let Ok(info) = napcat.get_login_info().await {
                evidence.uin = account_id(&info["data"]["uin"]);
                evidence.nickname = info["data"]["nick"].as_str().map(str::to_string);
            }
        }
        if evidence.logged_in
            && onebot
                .expected_account()
                .is_some_and(|expected| evidence.uin.as_deref() != Some(expected))
        {
            evidence.logged_in = false;
            evidence.source = "identity_mismatch".into();
        }
        return evidence;
    }
    LoginEvidence::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn failed_or_zero_onebot_identity_is_not_a_login() {
        assert!(
            !from_onebot(&json!({"status":"failed","retcode":1400,"data":{"user_id":10001}}))
                .logged_in
        );
        assert!(!from_onebot(&json!({"status":"ok","retcode":0,"data":{"user_id":0}})).logged_in);
        assert!(
            from_onebot(&json!({"status":"ok","retcode":0,"data":{"user_id":"10001"}})).logged_in
        );
    }
    #[test]
    fn webui_accepts_both_supported_login_fields() {
        assert!(from_webui(&json!({"code":0,"data":{"isLoggedIn":true,"uin":10001}})).logged_in);
        assert!(from_webui(&json!({"code":0,"data":{"isLogin":true,"uin":"10001"}})).logged_in);
        assert!(!from_webui(&json!({"code":-1,"data":{"isLogin":true}})).logged_in);
    }
    #[test]
    fn rejects_paths_and_shell_arguments_as_accounts() {
        for id in ["../10001", "10001 & taskkill", "0", "00001", "", "1000"] {
            assert!(validate_uin(id).is_err());
        }
    }
}
