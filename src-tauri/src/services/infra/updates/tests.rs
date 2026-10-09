use super::{download::*, release::*};
use serde_json::json;
use sha2::{Digest, Sha256};

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
