use super::cooldown::*;
use super::policy::*;
use super::trigger::*;
use serde_json::{json, Value};

fn policy_fixture(mode: &str, enabled: bool, summary: bool) -> TargetPolicy {
    TargetPolicy {
        exists: true,
        mode: mode.to_string(),
        enabled,
        is_summary_whitelist: summary,
        summary_interval_hours: 6,
        name: "t".to_string(),
        trigger_condition: "all".to_string(),
        keywords: Vec::new(),
        cooldown_seconds: 0,
    }
}

#[test]
fn default_policy_denies_everything() {
    let p = TargetPolicy::default();
    assert!(!p.exists);
    assert!(!p.tracked());
    assert!(!p.ai_execution_allowed());
}

#[test]
fn ignore_without_summary_is_fully_bypassed() {
    let p = policy_fixture("ignore", false, false);
    assert!(!p.tracked());
    assert!(!p.ai_execution_allowed());
}

#[test]
fn ignore_with_summary_whitelist_is_tracked_but_never_ai() {
    let p = policy_fixture("ignore", false, true);
    assert!(p.tracked());
    assert!(!p.ai_execution_allowed());
}

#[test]
fn enabled_ai_modes_execute() {
    for mode in ["auto_reply", "copilot"] {
        let p = policy_fixture(mode, true, false);
        assert!(p.tracked());
        assert!(p.ai_execution_allowed());
    }
}

#[test]
fn first_cooldown_call_is_allowed_and_second_is_blocked() {
    let target = "cooldown_test_basic";
    reset(target);
    assert!(try_acquire(target, 60).is_ok());
    let blocked = try_acquire(target, 60);
    assert!(blocked.is_err());
    reset(target);
}

#[test]
fn zero_cooldown_never_blocks() {
    let target = "cooldown_test_zero";
    reset(target);
    for _ in 0..5 {
        assert!(try_acquire(target, 0).is_ok());
    }
    assert_eq!(remaining_seconds(target, 0), 0);
    reset(target);
}

const SELF_ID: &str = "462564834";

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
    assert!(evaluate("all", &[], &plain, SELF_ID, "随便说点什么").is_matched());
}

#[test]
fn at_me_detects_structured_segments() {
    assert!(evaluate("at_me", &[], &at_event(SELF_ID), SELF_ID, "帮我看下").is_matched());
}

#[test]
fn keyword_condition_matches_case_insensitively() {
    let keywords = parse_keywords(r#"["EazyQQ","进度"]"#);
    let event = json!({ "raw_message": "eazyqq 现在什么进度" });
    assert!(evaluate("keyword", &keywords, &event, SELF_ID, "eazyqq 现在什么进度").is_matched());
}

#[test]
fn keyword_condition_skips_when_nothing_matches() {
    let keywords = parse_keywords(r#"["进度"]"#);
    let decision = evaluate("keyword", &keywords, &json!({}), SELF_ID, "今天天气不错");
    assert!(!decision.is_matched());
}
