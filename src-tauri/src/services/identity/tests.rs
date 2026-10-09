use super::accounts::*;
use super::bootstrap::*;
use super::machine::*;
use super::migration::*;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn delayed_identity_observation_cannot_reverse_an_explicit_selection() {
    let _guard = TEST_LOCK.lock().unwrap();
    let previous_active = active();
    let previous_bootstrap = read_bootstrap();
    clear_active().unwrap();
    assert!(adopt_observed(None,"10001").unwrap());
    assert!(select_for_restart("10002").unwrap());
    assert_eq!(active().as_deref(),Some("10001"),"The old process binding must stay immutable");
    let selected = std::fs::read(bootstrap_path()).unwrap();
    assert!(!adopt_observed(Some("10001"),"10001").unwrap());
    assert!(!adopt_observed(None,"10001").unwrap());
    assert_eq!(active().as_deref(),Some("10001"));
    assert_eq!(std::fs::read(bootstrap_path()).unwrap(),selected);
    // Another process may have selected a target while this context stayed unbound.
    set_active(None);
    assert!(!adopt_observed(None,"10001").unwrap());
    assert_eq!(std::fs::read(bootstrap_path()).unwrap(),selected);
    write_bootstrap(&previous_bootstrap).unwrap();
    set_active(previous_active);
}

#[test]
fn unbound_data_moves_into_the_adopted_account() {
    let _guard = TEST_LOCK.lock().unwrap();
    let uin = "555000111";
    let unbound = account_dir(None);
    let dest = account_dir(Some(uin));
    let _ = std::fs::remove_dir_all(&unbound);
    let _ = std::fs::remove_dir_all(&dest);

    std::fs::create_dir_all(unbound.join("logs")).unwrap();
    std::fs::write(unbound.join("eazyqq.db"), b"db").unwrap();
    std::fs::write(unbound.join("logs").join("eazyqq.log"), "x").unwrap();

    let moved = migrate_unbound_if_needed(uin);
    assert!(!moved.is_empty(), "pre-login data must be moved");
    assert!(dest.join("eazyqq.db").exists());
    assert!(dest.join("logs").join("eazyqq.log").exists());
    assert!(
        !unbound.exists(),
        "an emptied unbound directory should be removed"
    );

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn unbound_migration_is_a_noop_when_nothing_was_written() {
    let _guard = TEST_LOCK.lock().unwrap();
    let unbound = account_dir(None);
    let _ = std::fs::remove_dir_all(&unbound);
    assert!(migrate_unbound_if_needed("555000112").is_empty());
}

#[test]
fn sweeping_removes_the_qr_and_moves_logs() {
    let base = std::env::temp_dir().join("eazyqq_accounts_sweep");
    let _ = std::fs::remove_dir_all(&base);

    let napcat = base.join("napcat");
    std::fs::create_dir_all(napcat.join("logs")).unwrap();
    std::fs::create_dir_all(napcat.join("cache")).unwrap();
    std::fs::write(napcat.join("logs").join("session.log"), "聊天内容").unwrap();
    std::fs::write(napcat.join("cache").join("qrcode.png"), b"png").unwrap();

    let moved = sweep_napcat_artifacts(&napcat, "462564834");
    assert!(!moved.is_empty(), "something should have been swept");
    assert!(
        !napcat.join("cache").join("qrcode.png").exists(),
        "the login QR must not be left behind"
    );
    assert!(
        !napcat.join("logs").join("session.log").exists(),
        "logs containing message text must leave the shared directory"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sweeping_an_absent_directory_is_harmless() {
    let base = std::env::temp_dir().join("eazyqq_accounts_sweep_absent");
    let _ = std::fs::remove_dir_all(&base);
    assert!(sweep_napcat_artifacts(&base.join("nope"), "1").is_empty());
}

#[test]
fn bootstrap_round_trips_through_camel_case_json() {
    let text = r#"{"lastAccount":"462564834","webviewCompatMode":true}"#;
    let b: Bootstrap = serde_json::from_str(text).expect("parse");
    assert_eq!(b.last_account.as_deref(), Some("462564834"));
    assert!(b.webview_compat_mode);

    let written = serde_json::to_string(&b).expect("serialise");
    assert!(
        written.contains("lastAccount") && written.contains("webviewCompatMode"),
        "must write camelCase so other tools can read it: {written}"
    );
}

#[test]
fn unknown_or_missing_fields_fall_back_to_defaults() {
    let b: Bootstrap = serde_json::from_str("{}").expect("empty object is fine");
    assert!(b.last_account.is_none());
    assert!(!b.webview_compat_mode);

    let b: Bootstrap =
        serde_json::from_str(r#"{"lastAccount":"1","somethingElse":42}"#).expect("extra keys ok");
    assert_eq!(b.last_account.as_deref(), Some("1"));
}

#[test]
fn merge_moves_files_and_renames_collisions() {
    let base = std::env::temp_dir().join("eazyqq_accounts_merge");
    let _ = std::fs::remove_dir_all(&base);
    let src = base.join("src");
    let dst = base.join("dst");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();

    std::fs::write(dst.join("eazyqq.log"), "live").unwrap();
    std::fs::write(src.join("eazyqq.log"), "legacy").unwrap();
    std::fs::write(src.join("other.log"), "other").unwrap();

    let moved = merge_dir(&src, &dst).unwrap();
    assert_eq!(moved, 2, "both files must leave the shared directory");

    assert_eq!(
        std::fs::read_to_string(dst.join("eazyqq.log")).unwrap(),
        "live"
    );
    assert_eq!(
        std::fs::read_to_string(dst.join("other.log")).unwrap(),
        "other"
    );
    let legacy: Vec<_> = std::fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("legacy"))
        .collect();
    assert_eq!(
        legacy.len(),
        1,
        "the colliding file must be preserved under a new name"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn merge_leaves_nothing_behind_when_the_source_empties() {
    let base = std::env::temp_dir().join("eazyqq_accounts_merge2");
    let _ = std::fs::remove_dir_all(&base);
    let src = base.join("src");
    let dst = base.join("dst");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("a.txt"), "x").unwrap();

    merge_dir(&src, &dst).unwrap();
    assert!(
        !src.exists(),
        "an emptied source directory should be removed"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn account_ids_cannot_escape_the_accounts_directory() {
    for hostile in ["../../etc", "..\\..\\windows", "a/b", "a\\b"] {
        let dir = account_dir(Some(hostile));
        let s = dir.to_string_lossy().replace('\\', "/");
        assert!(
            s.contains("/accounts/") && !s.contains(".."),
            "{hostile} produced {s}"
        );
    }
}

#[test]
fn blank_account_falls_back_to_unbound() {
    assert!(account_dir(None).ends_with(UNBOUND));
    assert!(account_dir(Some("")).ends_with(UNBOUND));
    assert!(account_dir(Some("   ")).ends_with(UNBOUND));
}

#[test]
fn normal_account_gets_its_own_directory() {
    let a = account_dir(Some("462564834"));
    let b = account_dir(Some("1739677116"));
    assert_ne!(a, b, "different accounts must not share a directory");
    assert!(a.ends_with("462564834"));
    assert!(b.ends_with("1739677116"));
}

#[test]
fn path_is_keyed_by_machine_as_well_as_account() {
    let dir = account_dir(Some("462564834"));
    let s = dir.to_string_lossy().replace('\\', "/");
    assert!(
        s.contains(&format!("/accounts/{}/462564834", machine_id())),
        "expected a machine-keyed path, got {s}"
    );
}

#[test]
fn machine_id_is_stable_and_non_empty() {
    let a = machine_id();
    let b = machine_id();
    assert_eq!(a, b, "machine id must not change between calls");
    assert!(!a.is_empty());
    assert!(a.len() <= 32, "machine id should stay short: {a}");
    assert!(
        a.chars().all(|c| c.is_ascii_hexdigit()),
        "machine id should be a plain hex token: {a}"
    );
}

#[test]
fn fnv1a_matches_known_vectors() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
}

#[test]
fn different_machines_would_produce_different_directories() {
    assert_ne!(fnv1a64(b"machine-a"), fnv1a64(b"machine-b"));
}

#[test]
fn account_change_is_detected() {
    let _guard = TEST_LOCK.lock().unwrap();
    set_active(Some("111".to_string()));
    assert!(!account_changed("111"));
    assert!(
        account_changed("222"),
        "a different account must be reported"
    );

    set_active(None);
    assert!(
        !account_changed("222"),
        "unknown active account cannot 'change'"
    );
}

#[test]
fn active_account_round_trips() {
    let _guard = TEST_LOCK.lock().unwrap();
    set_active(Some("999".to_string()));
    assert_eq!(active().as_deref(), Some("999"));
    assert!(active_data_dir().ends_with("999"));

    set_active(None);
    assert!(active().is_none());
    assert!(active_data_dir().ends_with(UNBOUND));
}

#[test]
fn existing_database_never_adopts_another_databases_wal() {
    let _guard = TEST_LOCK.lock().unwrap();
    let unbound = account_dir(None);
    let dest = account_dir(Some("555000113"));
    std::fs::create_dir_all(&unbound).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(unbound.join("eazyqq.db"), b"source").unwrap();
    std::fs::write(unbound.join("eazyqq.db-wal"), b"source-wal").unwrap();
    std::fs::write(dest.join("eazyqq.db"), b"destination").unwrap();
    migrate_unbound_if_needed("555000113");
    assert!(!dest.join("eazyqq.db-wal").exists());
    assert_eq!(
        std::fs::read(dest.join("eazyqq.db")).unwrap(),
        b"destination"
    );
    assert_eq!(
        std::fs::read(unbound.join("eazyqq.db-wal")).unwrap(),
        b"source-wal"
    );
    std::fs::remove_dir_all(&unbound).unwrap();
    std::fs::remove_dir_all(&dest).unwrap();
}
