use super::boot::*;
use super::patch::*;

#[test]
fn electron_resolves_private_loader_from_qq_installation_without_modifying_qq() {
    let root = crate::services::logging::workspace_root().join("loader resolution 中文");
    let app = root.join("QQNT/versions/9.9.23-42430/resources/app");
    std::fs::create_dir_all(&app).unwrap();
    let original = br#"{"version":"9.9.23-42430","main":"./application.asar/app_launcher/index.js"}"#;
    std::fs::write(app.join("package.json"), original).unwrap();
    let qq = root.join("QQNT/QQ.exe");
    for uin in ["10001", "10002"] {
        let private = root.join(uin);
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("package.json"), r#"{"type":"module"}"#).unwrap();
        std::fs::write(private.join("loadNapCat.cjs"), format!("module.exports = '{uin}';")).unwrap();
        let patch = private.join("qqnt.json");
        sync_qqnt_patch(&qq, &patch).unwrap();
        let output = std::process::Command::new("node")
            .args(["-e", "const fs=require('node:fs');const p=JSON.parse(fs.readFileSync(process.argv[1]));process.stdout.write(require(require('node:path').join(process.cwd(),p.main)));", &patch.to_string_lossy()])
            .current_dir(&app).output().expect("Node is required by the frontend toolchain");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout), uin);
    }
    assert_eq!(std::fs::read(app.join("package.json")).unwrap(), original);
    assert!(!app.join("loadNapCat.js").exists());
}

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
fn boot_rate_limiting_and_repeated_failures() {
    note_healthy();
    note_attempt();
    let blocked = may_attempt();
    assert!(
        blocked.is_err(),
        "a second immediate attempt must be refused"
    );
    assert!(blocked.unwrap_err().contains("需等待"));

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
