use super::{download::*, release::*};
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn a_package_removed_after_preparation_leaves_the_same_failed_receipt() {
    use super::receipts::{Journal, Phase};
    let root = crate::services::logging::workspace_root().join("missing-prepared-installer");
    let bytes = b"MZisolated fixture; never executed";
    let path = root.join("setup.exe");
    let (journal, _) = Journal::begin_for(&root, "app", "install", Some("9.8.7".into()), Phase::Checking).unwrap();
    std::fs::write(&path, bytes).unwrap();
    journal.update(Phase::Verifying, None, None).unwrap(); journal.update(Phase::Ready, None, None).unwrap();
    let prepared = PreparedUpdate { operation_id: Some(journal.id().into()), path: path.clone(), sha256: format!("{:x}", Sha256::digest(bytes)), size: bytes.len() as u64, target_version: Some("9.8.7".into()) };
    std::fs::rename(&path, root.join("removed-package.bin")).unwrap();
    let error = super::installer::launch(&prepared, false).unwrap_err();
    assert!(error.contains(journal.id()));
    let history = super::receipts::history(&root).unwrap();
    assert_eq!(history.len(), 1); assert_eq!(history[0].operation_id, journal.id()); assert_eq!(history[0].phase, Phase::Failed);
    assert!(!root.join(format!("handoff-{}.json", journal.id())).exists());
}

#[cfg(windows)]
#[test]
fn handoff_worker() {
    if let Some(request) = std::env::args_os().find(|arg| std::path::Path::new(arg).extension().is_some_and(|extension| extension == "json")) {
        super::installer::run_helper(std::path::Path::new(&request)).unwrap();
        return;
    }
    let Some(path)=std::env::var_os("EAZYQQ_TEST_HANDOFF") else { return; };
    let path=std::path::PathBuf::from(path);
    let bytes=std::fs::read(&path).unwrap();
    let prepared=super::download::PreparedUpdate {operation_id:None,path,sha256:format!("{:x}",Sha256::digest(&bytes)),size:bytes.len() as u64,target_version:Some("9.8.7".into())};
    super::installer::launch(&prepared,false).unwrap();
}

#[cfg(windows)]
#[test]
fn installer_handoff_survives_caller_exit_and_handles_unicode_paths() {
    use std::os::windows::process::CommandExt;
    let root=crate::services::logging::workspace_root().join("installer handoff 中文");
    std::fs::create_dir_all(&root).unwrap();
    let source=root.join("fixture.c");let executable=root.join("fixture.exe");
    std::fs::write(&source,r#"
#include <windows.h>
#include <wchar.h>
int WINAPI WinMain(HINSTANCE a,HINSTANCE b,LPSTR command,int show){
  wchar_t file[32768];GetModuleFileNameW(NULL,file,32768);int fail=wcsstr(file,L"fixture-failure.exe")!=0;*wcsrchr(file,L'\\')=0;wcscat(file,L"\\installed.flag");
  HANDLE h=CreateFileW(file,GENERIC_WRITE,0,NULL,CREATE_ALWAYS,0,NULL);if(h==INVALID_HANDLE_VALUE)return 2;
  DWORD n;const wchar_t* args=GetCommandLineW();WriteFile(h,args,(DWORD)(wcslen(args)*2),&n,NULL);CloseHandle(h);return fail?9:0;
}
"#).unwrap();
    let compiled=std::process::Command::new("cl.exe").args(["/nologo","/MT"])
        .arg(&source).arg(format!("/Fe:{}",executable.display())).args(["/link","/SUBSYSTEM:WINDOWS","kernel32.lib"])
        .current_dir(&root).creation_flags(0x08000000).output().expect("MSVC is required by the Windows Rust toolchain");
    assert!(compiled.status.success(),"{}",String::from_utf8_lossy(&compiled.stdout));
    let child=std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact","services::infra::updates::tests::handoff_worker","--nocapture"])
        .env("EAZYQQ_TEST_HANDOFF",&executable).creation_flags(0x08000000).output().unwrap();
    assert!(child.status.success(),"{}",String::from_utf8_lossy(&child.stdout));
    let flag=root.join("installed.flag");let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
    while !flag.exists() && std::time::Instant::now()<deadline {std::thread::sleep(std::time::Duration::from_millis(150));}
    assert!(flag.exists(),"Detached installer did not run after caller exit: {:?}",std::fs::read_dir(&root).unwrap().map(|e|e.unwrap().file_name()).collect::<Vec<_>>());
    let bytes=std::fs::read(flag).unwrap();let wide=bytes.chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect::<Vec<_>>();
    let command=String::from_utf16(&wide).unwrap();assert!(command.contains("/P /R"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let final_receipt = loop {
        let values = super::receipts::history(&root).unwrap();
        if let Some(value) = values.into_iter().find(|value| value.phase == super::receipts::Phase::PendingVerification) { break value; }
        assert!(std::time::Instant::now() < deadline, "Installer exited without a durable success receipt");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(final_receipt.target_version.as_deref(), Some("9.8.7"));
    assert_eq!(final_receipt.installer_exit_code, Some(0));
    let failure_root = root.join("failure only");
    std::fs::create_dir_all(&failure_root).unwrap();
    let failure_executable = failure_root.join("fixture-failure.exe");
    std::fs::copy(executable, &failure_executable).unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "services::infra::updates::tests::handoff_worker", "--nocapture"])
        .env("EAZYQQ_TEST_HANDOFF", &failure_executable).creation_flags(0x08000000).output().unwrap();
    assert!(child.status.success(), "Caller could not hand off the simulated failure");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let failed = loop {
        let values = super::receipts::history(&failure_root).unwrap();
        if let Some(value) = values.into_iter().find(|value| value.phase == super::receipts::Phase::Failed) { break value; }
        assert!(std::time::Instant::now() < deadline, "Installer failure did not leave a durable result");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(failed.installer_exit_code, Some(9));
    assert!(failed.error.unwrap().contains("9"));
}

#[test]
fn unknown_core_version_and_missing_shell_are_distinct_states() {
    let release=json!({"tag_name":"v4.18.33","assets":[
        {"name":"NapCat.Framework.zip","browser_download_url":"https://example.test/framework.zip"},
        {"name":"NapCat.Shell.zip","browser_download_url":"https://github.com/NapNeko/NapCatQQ/releases/download/v4.18.33/NapCat.Shell.zip"}
    ]});
    let report=super::core::describe("unknown".into(),&release).unwrap().data.unwrap();
    assert_eq!(report.status,"version_unknown"); assert!(report.has_update);
    let report=super::core::describe("4.18.34".into(),&release).unwrap().data.unwrap();
    assert_eq!(report.status,"newer_local"); assert!(!report.has_update);
    let mut release=release; release["assets"]=json!([]);
    assert_eq!(super::core::describe("4.18.28".into(),&release).unwrap().data.unwrap().status,"asset_missing");
}

fn archive(module: &[u8], include_boot: bool) -> Vec<u8> {
    use std::io::Write;
    let mut writer=zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options=zip::write::SimpleFileOptions::default();
    let mut files=vec![("napcat.mjs",module),("config/webui.json",b"untrusted replacement".as_slice()),("loadNapCat.js",b"shared loader".as_slice())];
    if include_boot { files.extend([("NapCatWinBootMain.exe",b"MZfixture".as_slice()),("NapCatWinBootHook.dll",b"MZfixture".as_slice())]); }
    for (name,bytes) in files { writer.start_file(name,options).unwrap(); writer.write_all(bytes).unwrap(); }
    writer.finish().unwrap().into_inner()
}

#[test]
fn core_deployment_preserves_personal_configuration_and_keeps_backup() {
    let root=crate::services::logging::workspace_root().join("core-staging");
    let destination=root.join("napcat"); std::fs::create_dir_all(destination.join("config")).unwrap();
    std::fs::write(destination.join("napcat.mjs"),b"old module").unwrap();
    std::fs::write(destination.join("config/webui.json"),b"private token").unwrap();
    std::fs::write(destination.join("loadNapCat.js"),b"private loader").unwrap();
    let staging=root.join("updates");
    crate::services::infra::protocol_update::stage_and_install(&archive(b"new module",true),&staging,&destination).unwrap();
    assert_eq!(std::fs::read(destination.join("napcat.mjs")).unwrap(),b"new module");
    assert_eq!(std::fs::read(destination.join("config/webui.json")).unwrap(),b"private token");
    assert_eq!(std::fs::read(destination.join("loadNapCat.js")).unwrap(),b"private loader");
    let backup=std::fs::read_dir(staging).unwrap().next().unwrap().unwrap().path().join("backup/napcat.mjs");
    assert_eq!(std::fs::read(backup).unwrap(),b"old module");
}

#[test]
fn incomplete_core_archive_does_not_modify_existing_resources() {
    let root=crate::services::logging::workspace_root().join("incomplete-core");
    let destination=root.join("napcat"); std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("napcat.mjs"),b"original").unwrap();
    assert!(crate::services::infra::protocol_update::stage_and_install(&archive(b"bad new module",false),&root.join("updates"),&destination).is_err());
    assert_eq!(std::fs::read(destination.join("napcat.mjs")).unwrap(),b"original");
}

#[test]
fn core_deployment_records_a_durable_result_without_claiming_runtime_success() {
    use super::receipts::{Journal, Phase};
    let root = crate::services::logging::workspace_root().join("tracked-core-deployment");
    let destination = root.join("napcat");
    for (complete, expected) in [(true, Phase::Deployed), (false, Phase::Failed)] {
        let (journal, _) = Journal::begin_for(&root, "napcat", "install", Some("4.18.33".into()), Phase::Checking).unwrap();
        journal.update(Phase::Verifying, None, None).unwrap(); journal.update(Phase::Installing, None, None).unwrap();
        let result = super::core::deploy_tracked(&archive(b"new module", complete), &root.join("staging"), &destination, &journal);
        assert_eq!(result.is_ok(), complete); let receipt = journal.read().unwrap(); assert_eq!(receipt.phase, expected); assert!(receipt.verified_version.is_none());
        assert_eq!(std::fs::read(destination.join("napcat.mjs")).unwrap(), b"new module");
    }
}

#[test]
fn stable_releases_are_ordered_by_version_not_api_order() {
    let releases = json!([
        {"tag_name":"v9.0.0","draft":true}, {"tag_name":"v8.0.0-beta","prerelease":true},
        {"tag_name":"v0.4.0"}, {"tag_name":"v0.5.1"}, {"tag_name":"invalid"}
    ]);
    assert_eq!(select(&releases).unwrap()["tag_name"], "v0.5.1");
    assert!(select(&json!([{ "tag_name":"v0.6.0-beta"}])).is_none());
}

#[test]
fn cli_executables_and_foreign_urls_never_become_installers() {
    let release = json!({"tag_name":"v0.5.1", "assets":[
        {"name":"eazyqq_cli.exe", "browser_download_url":"https://github.com/LING71671/EazyQQ/releases/download/v0.5.1/eazyqq_cli.exe"},
        {"name":"EazyQQ_0.5.1_x64-setup.exe", "browser_download_url":"https://example.com/installer.exe"}
    ]});
    assert!(installer(&release).is_none());
    let mut release = release;
    release["assets"][1]["browser_download_url"] = json!("https://github.com/LING71671/EazyQQ/releases/download/v0.5.1/EazyQQ_0.5.1_x64-setup.exe");
    assert_eq!(installer(&release).unwrap()["name"], "EazyQQ_0.5.1_x64-setup.exe");
}

#[test]
fn checksum_matches_exact_asset_not_similar_filename() {
    let hash = "a".repeat(64);
    let body = format!("{hash}  eazyqq_cli.exe\n{hash}  EazyQQ_0.5.1_x64-setup.exe\n");
    assert_eq!(checksum_record(&body,"EazyQQ_0.5.1_x64-setup.exe"),Some(hash));
    assert!(checksum_record(&body,"EazyQQ_0.5.2_x64-setup.exe").is_none());
    assert!(!valid_hash(&"z".repeat(64)));
}

fn serve(body: &[u8]) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/installer.exe", listener.local_addr().unwrap());
    let body = body.to_vec();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0;4096]; let _ = stream.read(&mut request);
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
        stream.write_all(&body).unwrap();
    });
    url
}

#[tokio::test]
async fn corrupt_download_is_not_promoted_and_can_be_retried() {
    let root = crate::services::logging::workspace_root().join("corrupt-download");
    let good = b"MZexpected installer fixture";
    let bad = b"MZcorrupted installer bytes!";
    let mut artifact = Artifact { name:"setup.exe".into(), url:serve(bad),size:good.len() as u64,
        sha256:format!("{:x}",Sha256::digest(good)) };
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    assert!(download(&client,&artifact,&root,&|_| {}).await.is_err());
    assert!(!root.join("setup.exe").exists());
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry.unwrap().path().extension().is_some_and(|e| e == "part")));
    artifact.url=serve(good);
    let path=download(&client,&artifact,&root,&|_| {}).await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(),good);
}

#[tokio::test]
async fn tracked_download_correlates_hash_failure_and_a_separate_retry() {
    use super::receipts::{Operation, Phase};
    let root = crate::services::logging::workspace_root().join("tracked-download"); let good = b"MZverified installer";
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut ids = Vec::new();
    for (bytes, expected) in [(b"MZcorrupt installer!".as_slice(), Phase::Failed), (good.as_slice(), Phase::Ready)] {
        let mut operation = Operation::at(&root, "app", "install").unwrap(); ids.push(operation.journal().id().to_owned());
        let artifact = Artifact { name:"setup.exe".into(), url:serve(bytes), size:good.len() as u64, sha256:format!("{:x}", Sha256::digest(good)) };
        let result = download_tracked(&client, &artifact, &root, &|progress| { assert!(progress.operation_id.is_some()); }, operation.journal()).await;
        assert_eq!(operation.end(result, Phase::Ready).is_ok(), expected == Phase::Ready);
        let receipt = operation.journal().read().unwrap(); assert_eq!(receipt.phase, expected);
        assert!(receipt.events.iter().any(|event| event.phase == Phase::Verifying));
    }
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn cache_is_revalidated_and_html_never_passes_even_with_matching_hash() {
    let root=crate::services::logging::workspace_root().join("update-cache");
    let bytes=b"MZcached installer";
    let artifact=Artifact{name:"setup.exe".into(),url:"http://127.0.0.1:1/unreachable".into(),size:bytes.len() as u64,
        sha256:format!("{:x}",Sha256::digest(bytes))};
    std::fs::create_dir_all(&root).unwrap(); std::fs::write(root.join("setup.exe"),bytes).unwrap();
    assert!(download(&reqwest::Client::new(),&artifact,&root,&|_| {}).await.is_ok());
    std::fs::write(root.join("setup.exe"),b"MZmodified content").unwrap();
    assert!(verified_file(&root.join("setup.exe"),&artifact).is_err());
    let html=b"<html>rate limit</html>";
    let html_artifact=Artifact{name:"html.exe".into(),url:serve(html),size:html.len() as u64,sha256:format!("{:x}",Sha256::digest(html))};
    assert!(download(&reqwest::Client::builder().no_proxy().build().unwrap(),&html_artifact,&root,&|_|{}).await.is_err());
}
