use super::boot::*;
use super::patch::*;

#[test]
fn missing_install_reports_which_file_is_absent() {
    let dir = std::env::temp_dir().join("eazyqq_boot_test_missing");
    let _ = std::fs::create_dir_all(&dir);
    let err = resolve(&dir).unwrap_err();
    assert!(err.contains("NapCatWinBootMain.exe"), "got: {err}");
}

#[test]
fn empty_qq_path_file_is_rejected() {
    let dir = std::env::temp_dir().join("eazyqq_boot_test_empty");
    let cfg = dir.join("config");
    let _ = std::fs::create_dir_all(&cfg);
    std::fs::write(cfg.join("qq_path.txt"), "   \n").unwrap();

    let err = configured_qq_path(&dir).unwrap_err();
    assert!(err.contains("为空"), "got: {err}");
}

#[test]
fn qq_path_pointing_nowhere_is_rejected() {
    let dir = std::env::temp_dir().join("eazyqq_boot_test_nowhere");
    let cfg = dir.join("config");
    let _ = std::fs::create_dir_all(&cfg);
    std::fs::write(cfg.join("qq_path.txt"), r"Z:\definitely\not\here\QQ.exe").unwrap();

    let err = configured_qq_path(&dir).unwrap_err();
    assert!(err.contains("不存在"), "got: {err}");
}

#[test]
fn restart_is_rate_limited() {
    note_attempt();
    let blocked = may_attempt();
    assert!(blocked.is_err(), "a second immediate attempt must be refused");
    assert!(blocked.unwrap_err().contains("需等待"));
}

#[test]
fn repeated_failures_stop_the_loop() {
    for _ in 0..3 {
        note_result(false);
    }
    let blocked = may_attempt();
    assert!(blocked.is_err());
    assert!(blocked.unwrap_err().contains("停止自动重启"));
    assert_eq!(consecutive_failures(), 3);

    note_healthy();
    assert_eq!(consecutive_failures(), 0);
}
