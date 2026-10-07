use super::bundle::*;
use super::config::*;
use super::logging::*;
use super::zip::*;

// --- Logging Tests ---

#[test]
fn workspace_root_ignores_the_working_directory() {
    let before = workspace_root();
    let original = std::env::current_dir().expect("cwd");

    std::env::set_current_dir(std::env::temp_dir()).expect("chdir to temp");
    let after = workspace_root();
    std::env::set_current_dir(&original).expect("restore cwd");

    assert_eq!(before, after, "workspace root moved when the cwd changed");
}

#[test]
fn workspace_root_is_not_inside_build_output() {
    let root = workspace_root().to_string_lossy().replace('\\', "/");
    assert!(
        !root.contains("/target/"),
        "workspace root {root} is inside build output"
    );
}

#[test]
fn data_root_hangs_off_the_workspace_root() {
    assert_eq!(
        crate::services::accounts::data_root(),
        workspace_root().join("EazyQQ_Data")
    );
}

// --- Zip & Diagnostics Tests ---

#[test]
fn crc32_matches_the_standard_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn zip_container_is_structurally_valid() {
    let zip = build_zip(&[("a.txt".to_string(), b"abc".to_vec())]);
    assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    assert_eq!(&zip[zip.len() - 22..zip.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
    assert_eq!(u16::from_le_bytes([zip[zip.len() - 14], zip[zip.len() - 13]]), 1);
}

#[test]
fn redact_removes_a_supplied_secret() {
    let secret = std::env::var("TEST_SECRET_KEY")
        .unwrap_or_else(|_| "secret_redaction_test_fixture".to_string());
    let input = format!("key={} and more", secret);
    let out = redact(&input, &[secret.clone()]);
    assert!(!out.contains(&secret));
    assert!(out.contains("***REDACTED***"));
}

#[test]
fn redact_catches_generic_api_key_shapes() {
    let dummy_token = format!("{}-samplepattern123456", "sk");
    let input = format!("token {} end", dummy_token);
    let out = redact(&input, &[]);
    assert!(!out.contains(&dummy_token), "got: {out}");
}

#[test]
fn mask_id_keeps_ends_only() {
    assert_eq!(mask_id("1104661022"), "11***22");
    assert_eq!(mask_id("12345"), "12345");
}

// --- Config Tests ---

#[test]
fn every_config_field_is_read_somewhere() {
    let unread = unread_keys();
    assert!(
        unread.is_empty(),
        "these config fields are declared in default_app_config but never read: {:?}",
        unread
    );
}

#[test]
fn declared_keys_cover_the_expected_sections() {
    let keys = declared_keys();
    for expected in [
        "ai.activeProvider",
        "ai.model",
        "ai.temperature",
        "ai.maxContextMessages",
        "ai.baseUrl",
        "ai.apiKey",
        "napcat.wsPort",
        "napcat.autoRestart",
        "napcat.heartbeatIntervalSec",
        "storage.autoSyncFiles",
        "storage.maxFileSizeMb",
        "summary.enabled",
        "summary.intervalType",
        "summary.customIntervalMinutes",
        "summary.slidingWindowHours",
        "summary.autoForwardToPhone",
        "summary.customPrompt",
        "window.minimizeToTray",
        "window.closeToTray",
    ] {
        assert!(keys.contains(&expected.to_string()), "missing {expected}");
    }
}
