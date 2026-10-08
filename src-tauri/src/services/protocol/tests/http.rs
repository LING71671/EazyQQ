use crate::services::protocol::{session, NapCatService, OneBotClient};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(login: Value) -> Server {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", socket.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut client, _)) = socket.accept().await {
            let mut bytes = vec![0u8; 8192];
            let length = client.read(&mut bytes).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&bytes[..length]);
            let path = request
                .lines()
                .next()
                .unwrap_or("")
                .split_whitespace()
                .nth(1)
                .unwrap_or("");
            seen.lock().unwrap().push(path.to_string());
            let body = match path {
                "/get_login_info" => login.clone(),
                "/api/auth/login" => json!({"code":0,"data":{"Credential":"fixture-credential"}}),
                "/api/QQLogin/CheckLoginStatus" => json!({"code":0,"data":{"isLogin":false}}),
                "/api/QQLogin/RefreshQRcode" => {
                    json!({"code":0,"data":{"qrcodeurl":"https://example.test/fresh-qr"}})
                }
                _ => json!({"code":0,"data":{}}),
            }
            .to_string();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = client.write_all(response.as_bytes()).await;
        }
    });
    Server {
        base,
        requests,
        task,
    }
}

#[tokio::test]
async fn authenticated_onebot_overrides_stale_webui_without_triggering_login() {
    let server =
        server(json!({"status":"ok","retcode":0,"data":{"user_id":10001,"nickname":"fixture"}}))
            .await;
    let napcat = NapCatService::new(server.base.clone(), String::new(), String::new());
    let onebot = OneBotClient::for_account(server.base.clone(), Some("10001".into()));
    let result = session::probe(&napcat, &onebot).await;
    assert!(result.logged_in);
    assert_eq!(result.uin.as_deref(), Some("10001"));
    assert_eq!(
        server.requests.lock().unwrap().as_slice(),
        ["/get_login_info"]
    );
}

#[tokio::test]
async fn failed_envelope_and_wrong_account_are_not_accepted_as_authenticated() {
    let failed = server(json!({"status":"failed","retcode":1400,"data":{"user_id":10001}})).await;
    assert!(OneBotClient::new(failed.base.clone())
        .get_login_info()
        .await
        .is_err());
    let wrong = server(json!({"status":"ok","retcode":0,"data":{"user_id":10002}})).await;
    assert!(
        OneBotClient::for_account(wrong.base.clone(), Some("10001".into()))
            .get_login_info()
            .await
            .is_err()
    );
}

#[tokio::test]
async fn refresh_uses_the_actual_refresh_api_and_never_disk_cache() {
    let server = server(json!({"status":"failed","retcode":1400})).await;
    let napcat = NapCatService::new(server.base.clone(), String::new(), String::new());
    assert_eq!(
        napcat.refresh_qrcode().await.unwrap(),
        "https://example.test/fresh-qr"
    );
    assert!(server
        .requests
        .lock()
        .unwrap()
        .contains(&"/api/QQLogin/RefreshQRcode".to_string()));
    assert!(!server
        .requests
        .lock()
        .unwrap()
        .contains(&"/api/QQLogin/GetQQLoginQrcode".to_string()));
}

#[tokio::test]
async fn account_operations_reject_a_foreign_endpoint_before_roster_access() {
    let wrong = server(json!({"status":"ok","retcode":0,"data":{"user_id":10002}})).await;
    assert!(
        OneBotClient::for_account(wrong.base.clone(), Some("10001".into()))
            .get_friend_list()
            .await
            .is_err()
    );
    assert_eq!(
        wrong.requests.lock().unwrap().as_slice(),
        ["/get_login_info"]
    );
}
