use super::*;

#[test]
fn legacy_standalone_native_face_remains_a_single_bubble() {
    let plan = ReplyPlan::parse_candidate("[CQ:face,id=427]").unwrap();
    assert_eq!(plan.bubbles.len(), 1);
    assert_eq!(plan.bubbles[0].content(), "[CQ:face,id=427]");
}
use crate::services::{db::Database, protocol::OneBotClient};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn database(tag: &str) -> Database {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = crate::services::logging::workspace_root().join(format!(
        "reply-batch-{tag}-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Database::init(root.join("account/eazyqq.db")).unwrap()
}

fn plan(texts: &[&str]) -> ReplyPlan {
    ReplyPlan::from_bubble_texts(
        &texts
            .iter()
            .map(|text| text.to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn request(id: &str, plan: ReplyPlan) -> DeliveryRequest {
    DeliveryRequest {
        batch_id: id.into(),
        target_type: "private".into(),
        target_id: "20001".into(),
        source_message_id: "msg_20001_1".into(),
        input_snapshot: vec!["msg_20001_1".into()],
        authorization: "draft".into(),
        plan,
    }
}

enum Action {
    Ack(i64),
    Reject,
    MissingAck,
    Drop,
}
struct Stub {
    client: OneBotClient,
    sent: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Stub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn stub(actions: Vec<Action>) -> Stub {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = OneBotClient::for_account(
        format!("http://{}", listener.local_addr().unwrap()),
        Some("10001".into()),
    );
    let sent = Arc::new(Mutex::new(Vec::<Value>::new()));
    let observed = sent.clone();
    let actions = Arc::new(Mutex::new(VecDeque::from(actions)));
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut bytes = vec![];
            let header_end;
            loop {
                let mut chunk = [0u8; 4096];
                let size = socket.read(&mut chunk).await.unwrap();
                if size == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..size]);
                assert!(bytes.len() < 100_000);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        header_end = end + 4;
                        break;
                    }
                }
            }
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let path = headers
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            let body = if path == "/get_login_info" {
                json!({"status":"ok","retcode":0,"data":{"user_id":10001}})
            } else {
                assert_eq!(path, "/send_msg");
                observed
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&bytes[header_end..]).unwrap());
                match actions
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("Unexpected additional send")
                {
                    Action::Ack(id) => json!({"status":"ok","retcode":0,"data":{"message_id":id}}),
                    Action::Reject => {
                        json!({"status":"failed","retcode":1400,"wording":"rejected"})
                    }
                    Action::MissingAck => json!({"status":"ok","retcode":0,"data":{}}),
                    Action::Drop => continue,
                }
            }
            .to_string();
            let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    Stub { client, sent, task }
}

#[test]
fn explicit_bubbles_preserve_multiline_text_and_native_faces() {
    let legacy = ReplyPlan::parse_candidate("第一句\n第二句。还有一句").unwrap();
    assert_eq!(legacy.bubbles.len(), 1);
    let explicit=ReplyPlan::parse_candidate(r#"{"schemaVersion":1,"bubbles":[{"segments":[{"type":"text","text":"知道了"}]},{"segments":[{"type":"face","faceKey":"427"}]},{"segments":[{"type":"text","text":"明天说"},{"type":"face","faceKey":"324"}]}]}"#).unwrap();
    assert_eq!(explicit.bubbles.len(), 3);
    assert_eq!(explicit.bubbles[1].content(), "[CQ:face,id=427]");
    assert_eq!(explicit.bubbles[2].content(), "明天说[CQ:face,id=324]");
    assert!(
        ReplyPlan::parse_candidate(r#"{"schemaVersion":1,"targetId":"99999","bubbles":[]}"#)
            .is_err()
    );
    for invalid in [
        r#"{"schemaVersion":1,"bubbles":[{"segments":[{"type":"face","faceKey":"999999"}]}]}"#,
        r#"{"schemaVersion":1,"bubbles":[{"segments":[{"type":"text","text":"[CQ:at,qq=all]"}]}]}"#,
    ] {
        assert!(ReplyPlan::parse_candidate(invalid).is_err());
    }
    assert!(ReplyPlan::from_bubble_texts(&vec!["句子".into(); 7]).is_err());
}

#[tokio::test]
async fn retry_skips_confirmed_prefix_and_keeps_each_bubble_atomic() {
    let db = database("partial");
    let server = stub(vec![
        Action::Ack(10),
        Action::Reject,
        Action::Ack(20),
        Action::Ack(30),
    ])
    .await;
    let plan = plan(&["第一条[CQ:face,id=324]", "[CQ:face,id=427]", "第三条"]);
    let first = deliver(
        &db,
        &server.client,
        request("partial", plan.clone()),
        |_| async { Ok(true) },
    )
    .await
    .unwrap();
    assert_eq!(first.status, "partial");
    assert_eq!(
        first
            .receipts
            .iter()
            .map(|r| r.status.as_str())
            .collect::<Vec<_>>(),
        ["sent", "failed", "pending"]
    );
    let second = deliver(
        &db,
        &server.client,
        request("partial", plan),
        |ids| async move {
            assert!(ids.contains(&"msg_20001_10".to_string()));
            Ok(true)
        },
    )
    .await
    .unwrap();
    assert_eq!(second.status, "sent");
    assert_eq!(server.sent.lock().unwrap().len(), 4);
    assert_eq!(
        server.sent.lock().unwrap()[0]["message"],
        json!([{"type":"text","data":{"text":"第一条"}},{"type":"face","data":{"id":"324"}}])
    );
    assert_eq!(
        server.sent.lock().unwrap()[2]["message"],
        json!([{"type":"face","data":{"id":"427"}}])
    );
    assert_eq!(db.get_messages_by_target("20001", 20).unwrap().len(), 3);
}

#[tokio::test]
async fn missing_ack_blocks_retry_and_new_batch_until_explicit_reconciliation() {
    let db = database("unknown");
    let server = stub(vec![Action::MissingAck, Action::Ack(20)]).await;
    let plan = plan(&["第一条"]);
    assert_eq!(
        deliver(
            &db,
            &server.client,
            request("unknown", plan.clone()),
            |_| async { Ok(true) }
        )
        .await
        .unwrap()
        .status,
        "unknown"
    );
    assert!(db.get_messages_by_target("20001", 20).unwrap().is_empty());
    assert_eq!(
        deliver(
            &db,
            &server.client,
            request("unknown", plan.clone()),
            |_| async { Ok(true) }
        )
        .await
        .unwrap()
        .status,
        "unknown"
    );
    assert!(deliver(
        &db,
        &server.client,
        request("bypass", plan.clone()),
        |_| async { Ok(true) }
    )
    .await
    .is_err());
    assert_eq!(server.sent.lock().unwrap().len(), 1);
    reconcile(&db, "unknown", "b1", None, true).unwrap();
    assert_eq!(
        deliver(&db, &server.client, request("unknown", plan), |_| async {
            Ok(true)
        })
        .await
        .unwrap()
        .status,
        "sent"
    );
    assert_eq!(server.sent.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn confirmed_delivered_reconciliation_caches_message_and_only_sends_remainder() {
    let db = database("reconcile");
    let server = stub(vec![Action::Drop, Action::Ack(20)]).await;
    let plan = plan(&["第一条", "第二条"]);
    let first = deliver(
        &db,
        &server.client,
        request("reconcile", plan.clone()),
        |_| async { Ok(true) },
    )
    .await
    .unwrap();
    assert_eq!(first.status, "unknown");
    let report = reconcile(&db, "reconcile", "b1", Some("10"), false).unwrap();
    assert_eq!(report.receipts[0].status, "sent");
    assert!(db.get_message_source("msg_20001_10").unwrap().is_some());
    assert_eq!(
        deliver(&db, &server.client, request("reconcile", plan), |_| async {
            Ok(true)
        })
        .await
        .unwrap()
        .status,
        "sent"
    );
    assert_eq!(server.sent.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn changed_authorization_stops_remaining_bubbles_and_plan_cannot_change_after_prefix() {
    let db = database("revoked");
    let server = stub(vec![Action::Ack(10)]).await;
    let plan = plan(&["第一条", "第二条"]);
    let report = deliver(
        &db,
        &server.client,
        request("revoked", plan.clone()),
        |ids| async move { Ok(ids.is_empty()) },
    )
    .await
    .unwrap();
    assert_eq!(report.status, "superseded");
    assert_eq!(report.receipts[1].status, "superseded");
    assert_eq!(server.sent.lock().unwrap().len(), 1);
    assert!(deliver(
        &db,
        &server.client,
        request("revoked", super::tests::plan(&["篡改内容"])),
        |_| async { Ok(true) }
    )
    .await
    .is_err());
    let mut rebound = request("revoked", plan);
    rebound.input_snapshot.push("new incoming".into());
    assert!(
        deliver(&db, &server.client, rebound, |_| async { Ok(true) })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn final_guard_after_identity_stops_before_the_first_network_write() {
    let db = database("final-guard");
    let server = stub(vec![]).await;
    let checks = AtomicUsize::new(0);
    let report = deliver(
        &db,
        &server.client,
        request("final-guard", plan(&["不会发送"])),
        |_| {
            let allowed = checks.fetch_add(1, Ordering::Relaxed) == 0;
            async move { Ok(allowed) }
        },
    )
    .await
    .unwrap();
    assert_eq!(report.status, "superseded");
    assert_eq!(report.receipts[0].status, "superseded");
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert!(server.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cross_process_lease_excludes_concurrent_delivery_and_interrupted_write_is_unknown() {
    let db = database("lease");
    let server = stub(vec![]).await;
    let plan = plan(&["第一条"]);
    let candidate = super::store::initial(
        "crash".into(),
        "10001".into(),
        "private".into(),
        "20001".into(),
        "msg_20001_1".into(),
        vec!["msg_20001_1".into()],
        "draft".into(),
        plan.clone(),
    );
    let mut batch = super::store::claim(&db, candidate, "first-process").unwrap();
    batch.report.receipts[0].status = "sending".into();
    super::store::save(&db, &mut batch, "first-process").unwrap();
    let path = db
        .with_connection(|conn| {
            conn.query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        })
        .unwrap();
    let other_process = Database::init(path).unwrap();
    assert!(deliver(
        &other_process,
        &server.client,
        request("crash", plan.clone()),
        |_| async { Ok(true) }
    )
    .await
    .is_err());
    assert!(deliver(
        &other_process,
        &server.client,
        request("parallel", plan.clone()),
        |_| async { Ok(true) }
    )
    .await
    .is_err());
    db.with_connection(|conn|conn.execute_batch("UPDATE reply_delivery_batches SET lease_until=0;UPDATE reply_delivery_targets SET lease_until=0;")).unwrap();
    assert_eq!(
        get_batch(&other_process, "crash")
            .unwrap()
            .unwrap()
            .report
            .receipts[0]
            .status,
        "unknown"
    );
    let resumed = deliver(
        &other_process,
        &server.client,
        request("crash", plan),
        |_| async { Ok(true) },
    )
    .await
    .unwrap();
    assert_eq!(resumed.receipts[0].status, "unknown");
    assert!(server.sent.lock().unwrap().is_empty());
}

async fn delayed_model() -> (
    String,
    tokio::sync::mpsc::UnboundedReceiver<tokio::sync::oneshot::Sender<String>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let sender = sender.clone();
            tokio::spawn(async move {
                let mut bytes = vec![];
                loop {
                    let mut chunk = [0u8; 4096];
                    let count = socket.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    assert!(bytes.len() < 200_000);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.split_once(':')
                                    .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let (respond, content) = tokio::sync::oneshot::channel::<String>();
                if sender.send(respond).is_err() {
                    return;
                }
                let Ok(content) = content.await else {
                    return;
                };
                let body = json!({"choices":[{"message":{"role":"assistant","content":content}}]})
                    .to_string();
                let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
            });
        }
    });
    (endpoint, receiver, task)
}

fn revision_fixture(
    tag: &str,
    endpoint: String,
) -> (
    Arc<Database>,
    Arc<OneBotClient>,
    Arc<crate::services::ai::AiService>,
) {
    let db = Arc::new(database(tag));
    db.upsert_rule(&crate::services::storage::ContactRuleRecord {
        target_id: "20001".into(),
        target_type: "private".into(),
        name: "peer".into(),
        avatar_url: String::new(),
        mode: "draft".into(),
        trigger_condition: "all".into(),
        keywords: "[]".into(),
        cooldown_seconds: 0,
        enabled: true,
        is_summary_whitelist: false,
        summary_interval_hours: 6,
        updated_at: 0,
    })
    .unwrap();
    db.save_draft(&crate::models::PendingDraftDto {
        id: "revision".into(),
        target_id: "20001".into(),
        target_name: "peer".into(),
        target_type: "private".into(),
        reply_to_msg_id: "legacy".into(),
        incoming_message_snippet: "原消息".into(),
        generated_content: "旧草稿".into(),
        reply_plan: None,
        thinking_content: None,
        model_used: "mock-model".into(),
        created_at: 1,
    })
    .unwrap();
    let client = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:9".into(),
        Some("10001".into()),
    ));
    let config = crate::services::ai::AiRuntimeConfig::from_app_config(
        Some(&json!({"activeProvider":"custom","baseUrl":endpoint,"model":"mock-model"})),
        "",
    );
    let directory = db
        .with_connection(|conn| {
            conn.query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        })
        .unwrap();
    let ai = Arc::new(crate::services::ai::AiService::for_account(
        config,
        std::path::Path::new(&directory)
            .parent()
            .unwrap()
            .to_path_buf(),
    ));
    (db, client, ai)
}

#[tokio::test]
async fn concurrent_revisions_reserve_versions_before_await_and_only_latest_can_commit() {
    use crate::services::protocol::reply_workflow;
    let (endpoint, mut requests, server) = delayed_model().await;
    let (db, client, ai) = revision_fixture("revision-race", endpoint);
    let first = {
        let db = db.clone();
        let client = client.clone();
        let ai = ai.clone();
        tokio::spawn(async move {
            reply_workflow::regenerate(&db, &client, &ai, "revision", "first").await
        })
    };
    let first_response = tokio::time::timeout(std::time::Duration::from_secs(5), requests.recv())
        .await
        .unwrap()
        .unwrap();
    let reserved: Value =
        serde_json::from_str(&db.get_setting("draft_context:revision").unwrap().unwrap()).unwrap();
    assert_eq!(reserved["generation"], 1);
    assert_eq!(reserved["generating"], true);
    assert!(
        reply_workflow::approve_draft(&db, &client, "revision", None, None)
            .await
            .unwrap_err()
            .contains("修订")
    );
    let second = {
        let db = db.clone();
        let client = client.clone();
        let ai = ai.clone();
        tokio::spawn(async move {
            reply_workflow::regenerate(&db, &client, &ai, "revision", "second").await
        })
    };
    let second_response = tokio::time::timeout(std::time::Duration::from_secs(5), requests.recv())
        .await
        .unwrap()
        .unwrap();
    second_response
        .send(serde_json::to_string(&plan(&["新结果"])).unwrap())
        .unwrap();
    assert_eq!(second.await.unwrap().unwrap().generated_content, "新结果");
    first_response
        .send(serde_json::to_string(&plan(&["旧结果"])).unwrap())
        .unwrap();
    assert!(first.await.unwrap().is_err());
    assert_eq!(
        db.get_pending_drafts().unwrap()[0].generated_content,
        "新结果"
    );
    let metadata: Value =
        serde_json::from_str(&db.get_setting("draft_context:revision").unwrap().unwrap()).unwrap();
    assert_eq!(metadata["generation"], 2);
    assert_eq!(metadata["generating"], false);
    server.abort();
}

#[tokio::test]
async fn revision_commit_rechecks_delivery_lease_acquired_while_model_was_running() {
    use crate::services::protocol::reply_workflow;
    let (endpoint, mut requests, server) = delayed_model().await;
    let (db, client, ai) = revision_fixture("revision-delivery-race", endpoint);
    let revision = {
        let db = db.clone();
        let client = client.clone();
        let ai = ai.clone();
        tokio::spawn(async move {
            reply_workflow::regenerate(&db, &client, &ai, "revision", "change").await
        })
    };
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), requests.recv())
        .await
        .unwrap()
        .unwrap();
    let candidate = super::store::initial(
        "revision".into(),
        "10001".into(),
        "private".into(),
        "20001".into(),
        "legacy".into(),
        vec![],
        "draft".into(),
        plan(&["旧草稿"]),
    );
    let batch = super::store::claim(&db, candidate, "another-process").unwrap();
    response
        .send(serde_json::to_string(&plan(&["被阻止的覆盖"])).unwrap())
        .unwrap();
    assert!(revision.await.unwrap().is_err());
    assert_eq!(
        db.get_pending_drafts().unwrap()[0].generated_content,
        "旧草稿"
    );
    super::store::release(&db, &batch, "another-process");
    server.abort();
}

#[test]
fn retention_archives_only_terminal_reports_and_tombstones_prevent_replay() {
    let db = database("retention");
    let base = plan(&["已完成"]);
    // Initialize the account-local delivery schema without creating a send attempt.
    assert!(super::store::get(&db, "none").unwrap().is_none());
    db.with_connection(|connection| {
        let transaction=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for index in 0..250 {
            let mut batch=super::store::initial(format!("terminal_{index:03}"),"10001".into(),"private".into(),format!("{}",30_000+index),String::new(),vec![],"draft".into(),base.clone());
            batch.report.status="sent".into();batch.report.receipts[0].status="sent".into();batch.report.receipts[0].message_id=Some(format!("{}",index+1));batch.updated_at=index;
            transaction.execute("INSERT INTO reply_delivery_batches(batch_id,account_uin,target_type,target_id,body_json) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![batch.batch_id,batch.account_uin,batch.target_type,batch.target_id,serde_json::to_string(&batch).unwrap()])?;
        }
        for (index,state) in ["unknown","pending","failed","sending","active"].iter().enumerate() {
            let mut batch=super::store::initial(format!("protected_{state}"),"10001".into(),"private".into(),format!("{}",90_000+index),String::new(),vec![],"draft".into(),base.clone());
            batch.report.status=if *state=="active" {"sent"} else {state}.to_string();batch.report.receipts[0].status=batch.report.status.clone();
            let lease=if *state=="active" {super::store::now_ms()+30_000} else {0};
            transaction.execute("INSERT INTO reply_delivery_batches(batch_id,account_uin,target_type,target_id,body_json,lease_until) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![batch.batch_id,batch.account_uin,batch.target_type,batch.target_id,serde_json::to_string(&batch).unwrap(),lease])?;
        }
        transaction.commit()
    }).unwrap();
    // Read paths recover ambiguity but never compact terminal history.
    assert!(super::store::get(&db, "terminal_000").unwrap().is_some());
    let before: usize = db
        .with_connection(|connection| {
            connection.query_row("SELECT COUNT(*) FROM reply_delivery_batches", [], |row| {
                row.get(0)
            })
        })
        .unwrap();
    assert_eq!(before, 255);
    let next = super::store::initial(
        "next".into(),
        "10001".into(),
        "private".into(),
        "20001".into(),
        String::new(),
        vec![],
        "draft".into(),
        base.clone(),
    );
    let claimed = super::store::claim(&db, next, "new-owner").unwrap();
    super::store::release(&db, &claimed, "new-owner");
    let (full,proofs):(usize,usize)=db.with_connection(|connection|connection.query_row("SELECT (SELECT COUNT(*) FROM reply_delivery_batches),(SELECT COUNT(*) FROM reply_delivery_tombstones)",[],|row|Ok((row.get(0)?,row.get(1)?)))).unwrap();
    assert_eq!(full, 206);
    assert_eq!(proofs, 50);
    assert!(super::store::get(&db, "terminal_000").unwrap().is_none());
    assert!(super::store::is_completed_batch(&db, "terminal_000").unwrap());
    assert!(super::store::is_completed_batch(&db, "terminal_249").unwrap());
    for state in ["unknown", "pending", "failed", "sending", "active"] {
        assert!(super::store::get(&db, &format!("protected_{state}"))
            .unwrap()
            .is_some());
    }
    let replay = super::store::initial(
        "terminal_000".into(),
        "10001".into(),
        "private".into(),
        "30000".into(),
        String::new(),
        vec![],
        "draft".into(),
        base,
    );
    assert!(super::store::claim(&db, replay, "replay")
        .unwrap_err()
        .contains("归档"));
}

#[test]
fn unresolved_report_and_tombstone_caps_refuse_new_batches_without_blocking_existing_recovery() {
    let db = database("retention-caps");
    let base = plan(&["待核对"]);
    let _ = super::store::get(&db, "none").unwrap();
    db.with_connection(|connection| {
        let transaction=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for index in 0..500 {
            let batch=super::store::initial(format!("pending_{index}"),"10001".into(),"private".into(),format!("{}",30_000+index),String::new(),vec![],"draft".into(),base.clone());
            transaction.execute("INSERT INTO reply_delivery_batches(batch_id,account_uin,target_type,target_id,body_json) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![batch.batch_id,batch.account_uin,batch.target_type,batch.target_id,serde_json::to_string(&batch).unwrap()])?;
        }
        transaction.commit()
    }).unwrap();
    let next = super::store::initial(
        "overflow".into(),
        "10001".into(),
        "private".into(),
        "20001".into(),
        String::new(),
        vec![],
        "draft".into(),
        base.clone(),
    );
    assert!(super::store::claim(&db, next.clone(), "new")
        .unwrap_err()
        .contains("500条"));
    let existing = super::store::get(&db, "pending_0").unwrap().unwrap();
    let resumed = super::store::claim(&db, existing.clone(), "retry").unwrap();
    super::store::release(&db, &resumed, "retry");
    db.with_connection(|connection|connection.execute_batch("WITH RECURSIVE sequence(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM sequence WHERE n<50000) INSERT INTO reply_delivery_tombstones(batch_id,account_uin,target_type,target_id,plan_hash,terminal_status,archived_at) SELECT 'old_'||n,'10001','private','20001','fixed-hash','sent',0 FROM sequence;")).unwrap();
    assert!(super::store::claim(&db, next, "new")
        .unwrap_err()
        .contains("50000条"));
    let resumed = super::store::claim(&db, existing, "retry-again").unwrap();
    super::store::release(&db, &resumed, "retry-again");
    let count: usize = db
        .with_connection(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM reply_delivery_tombstones",
                [],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert_eq!(count, 50_000);
}
