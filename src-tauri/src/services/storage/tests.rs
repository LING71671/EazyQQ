use std::fs;
use crate::models::MessageItemDto;
use super::db::Database;

fn temp_db(tag: &str) -> Database {
    let dir = std::env::temp_dir().join("eazyqq_db_tests");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{}_{}.db", tag, std::process::id()));
    let _ = fs::remove_file(&path);
    Database::init(&path).expect("temp db")
}

fn msg(id: &str, target: &str, ts: i64, from_me: bool) -> MessageItemDto {
    MessageItemDto {
        id: id.to_string(),
        target_id: target.to_string(),
        sender_id: if from_me { "me".into() } else { "10001".into() },
        sender_name: if from_me { "我".into() } else { "对方".into() },
        content: format!("内容_{}", id),
        is_from_me: from_me,
        ai_reply_status: "none".to_string(),
        timestamp: ts,
    }
}

#[test]
fn unread_counts_only_incoming_messages() {
    let db = temp_db("unread_incoming");
    db.save_message(&msg("m1", "g1", 1000, false)).unwrap();
    db.save_message(&msg("m2", "g1", 2000, true)).unwrap();
    db.save_message(&msg("m3", "g1", 3000, false)).unwrap();

    assert_eq!(db.count_unread("g1").unwrap(), 2, "our own message must not count");
}

#[test]
fn unread_is_scoped_per_target() {
    let db = temp_db("unread_scope");
    db.save_message(&msg("a1", "g1", 1000, false)).unwrap();
    db.save_message(&msg("b1", "g2", 1000, false)).unwrap();

    assert_eq!(db.count_unread("g1").unwrap(), 1);
    assert_eq!(db.count_unread("g2").unwrap(), 1);
    assert_eq!(db.count_unread("g3").unwrap(), 0);
}

#[test]
fn marking_read_clears_the_badge() {
    let db = temp_db("unread_mark");
    db.save_message(&msg("m1", "g1", 1000, false)).unwrap();
    db.save_message(&msg("m2", "g1", 2000, false)).unwrap();
    assert_eq!(db.count_unread("g1").unwrap(), 2);

    db.mark_read("g1", 2000).unwrap();
    assert_eq!(db.count_unread("g1").unwrap(), 0);

    // A newer message starts counting again.
    db.save_message(&msg("m3", "g1", 3000, false)).unwrap();
    assert_eq!(db.count_unread("g1").unwrap(), 1);
}

#[test]
fn mark_read_never_moves_backwards() {
    let db = temp_db("unread_monotonic");
    db.save_message(&msg("m1", "g1", 5000, false)).unwrap();

    db.mark_read("g1", 5000).unwrap();
    assert_eq!(db.count_unread("g1").unwrap(), 0);

    // An out-of-order (older) mark must not resurrect already-read messages.
    db.mark_read("g1", 1000).unwrap();
    assert_eq!(db.get_last_read("g1").unwrap(), 5000);
    assert_eq!(db.count_unread("g1").unwrap(), 0);
}

#[test]
fn last_read_defaults_to_zero_for_an_unseen_target() {
    let db = temp_db("unread_default");
    assert_eq!(db.get_last_read("never-opened").unwrap(), 0);
    assert_eq!(db.count_unread("never-opened").unwrap(), 0);
}

#[test]
fn messages_for_a_target_are_returned_newest_last() {
    let db = temp_db("history_order");
    for ts in [1000, 3000, 2000] {
        db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
    }

    let history = db.get_messages_by_target("g1", 10).unwrap();
    let stamps: Vec<i64> = history.iter().map(|m| m.timestamp).collect();
    assert_eq!(stamps, vec![1000, 2000, 3000], "must be chronological");
}

#[test]
fn history_limit_keeps_the_newest_messages() {
    let db = temp_db("history_limit");
    for ts in [1000, 2000, 3000, 4000, 5000] {
        db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
    }

    let history = db.get_messages_by_target("g1", 2).unwrap();
    let stamps: Vec<i64> = history.iter().map(|m| m.timestamp).collect();
    assert_eq!(stamps, vec![4000, 5000], "the window must keep the newest, not the oldest");
}

#[test]
fn window_query_respects_the_lower_bound() {
    let db = temp_db("window");
    for ts in [1000, 2000, 3000, 4000] {
        db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
    }

    let window = db.get_messages_in_window("g1", 2500, 100).unwrap();
    let stamps: Vec<i64> = window.iter().map(|m| m.timestamp).collect();
    assert_eq!(stamps, vec![3000, 4000]);
}

#[test]
fn default_rule_is_not_created_for_unknown_targets() {
    let db = temp_db("rules_empty");
    assert!(db.get_all_rules().unwrap().is_empty());
    assert!(db.get_summary_whitelist_groups().unwrap().is_empty());
}
