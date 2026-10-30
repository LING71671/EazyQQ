use super::ReplyPlan;
use crate::models::PendingDraftDto;
use crate::services::{
    db::Database,
    protocol::{
        context::{self, ConversationScope},
        reply_workflow, OneBotClient,
    },
    storage::ContactRuleRecord,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn fixture(tag: &str) -> (Arc<Database>, ConversationScope) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = crate::services::logging::workspace_root().join(format!(
        "reply-workflow-{tag}-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let db = Arc::new(Database::init(root.join("account/eazyqq.db")).unwrap());
    db.upsert_rule(&ContactRuleRecord {
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
    (
        db,
        ConversationScope {
            account_uin: "10001".into(),
            target_type: "private".into(),
            target_id: "20001".into(),
        },
    )
}

fn save_incoming(db: &Database, scope: &ConversationScope, id: i64) -> String {
    let mut raw = json!({"self_id":10001,"message_id":id,"message_type":scope.target_type,"user_id":20001,"time":id,"message":"来信"});
    if scope.target_type == "group" {
        raw["group_id"] = json!(scope.target_id);
        raw["user_id"] = json!(30001);
    }
    let message = context::message_from_protocol(scope, &raw, 0).unwrap();
    context::store_message(
        db,
        scope,
        &message,
        &context::source_record(scope, "peer_live", &raw),
        true,
    )
    .unwrap();
    message.id
}

fn draft(source_id: String) -> PendingDraftDto {
    let plan = ReplyPlan::from_bubble_texts(&[
        "第一条".into(),
        "[CQ:face,id=427]".into(),
        "第三条[CQ:face,id=324]".into(),
    ])
    .unwrap();
    PendingDraftDto {
        id: "review".into(),
        target_id: "20001".into(),
        target_name: "peer".into(),
        target_type: "private".into(),
        reply_to_msg_id: source_id,
        incoming_message_snippet: "来信".into(),
        generated_content: plan.preview(),
        reply_plan: Some(plan),
        thinking_content: None,
        model_used: "mock".into(),
        created_at: 1000,
    }
}

async fn server(
    db: Arc<Database>,
    scope: ConversationScope,
    new_incoming_after_first: bool,
) -> (
    OneBotClient,
    Arc<Mutex<Vec<Value>>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = OneBotClient::for_account(
        format!("http://{}", listener.local_addr().unwrap()),
        Some("10001".into()),
    );
    let sent = Arc::new(Mutex::new(vec![]));
    let captured = sent.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut bytes = vec![];
            let header_end;
            loop {
                let mut chunk = [0u8; 4096];
                let count = socket.read(&mut chunk).await.unwrap();
                if count == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..count]);
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
            let data = match path {
                "/get_login_info" => json!({"user_id":10001}),
                "/get_friend_msg_history" | "/get_group_msg_history" => json!({"messages":[]}),
                "/send_msg" => {
                    let count = {
                        let mut captured = captured.lock().unwrap();
                        captured.push(serde_json::from_slice(&bytes[header_end..]).unwrap());
                        captured.len()
                    };
                    if count == 1 && new_incoming_after_first {
                        save_incoming(&db, &scope, 2);
                    }
                    json!({"message_id":100+count})
                }
                other => panic!("Unexpected mock protocol request: {other}"),
            };
            let body = json!({"status":"ok","retcode":0,"data":data}).to_string();
            let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
        }
    });
    (client, sent, task)
}

#[tokio::test]
async fn saved_typed_draft_approval_sends_native_bubbles_and_cleans_only_completed_draft() {
    let (db, scope) = fixture("approve");
    let source = save_incoming(&db, &scope, 1);
    let (client, sent, task) = server(db.clone(), scope.clone(), false).await;
    let draft = draft(source);
    let snapshot = reply_workflow::input_snapshot(&db, &scope, &draft.reply_to_msg_id, &[])
        .unwrap()
        .unwrap();
    reply_workflow::save_plan_draft(&db, &client, &draft, snapshot, 0).unwrap();
    let report = reply_workflow::approve_draft(&db, &client, &draft.id, None, None)
        .await
        .unwrap();
    assert_eq!(report.status, "sent");
    assert_eq!(report.receipts.len(), 3);
    assert!(db.get_pending_drafts().unwrap().is_empty());
    assert_eq!(sent.lock().unwrap().len(), 3);
    assert_eq!(
        sent.lock().unwrap()[1]["message"],
        json!([{"type":"face","data":{"id":"427"}}])
    );
    assert_eq!(
        sent.lock().unwrap()[2]["message"],
        json!([{"type":"text","data":{"text":"第三条"}},{"type":"face","data":{"id":"324"}}])
    );
    assert!(db.get_message_source("msg_20001_103").unwrap().is_some());
    assert_eq!(
        super::get_batch(&db, &draft.id)
            .unwrap()
            .unwrap()
            .report
            .status,
        "sent"
    );
    task.abort();
}

#[tokio::test]
async fn new_verified_incoming_between_bubbles_stops_remainder_and_keeps_reviewable_receipts() {
    let (db, scope) = fixture("new-incoming");
    let source = save_incoming(&db, &scope, 1);
    let (client, sent, task) = server(db.clone(), scope.clone(), true).await;
    let draft = draft(source);
    let snapshot = reply_workflow::input_snapshot(&db, &scope, &draft.reply_to_msg_id, &[])
        .unwrap()
        .unwrap();
    reply_workflow::save_plan_draft(&db, &client, &draft, snapshot, 0).unwrap();
    let report = reply_workflow::approve_draft(&db, &client, &draft.id, None, None)
        .await
        .unwrap();
    assert_eq!(report.status, "superseded");
    assert_eq!(report.receipts[0].status, "sent");
    assert_eq!(report.receipts[1].status, "superseded");
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert_eq!(db.get_pending_drafts().unwrap().len(), 1);
    assert!(db.get_message_source("msg_20001_101").unwrap().is_some());
    let retry = reply_workflow::approve_draft(&db, &client, &draft.id, None, None)
        .await
        .unwrap();
    assert_eq!(retry.status, "superseded");
    assert_eq!(sent.lock().unwrap().len(), 1);
    task.abort();
}

#[tokio::test]
async fn same_numeric_target_with_foreign_group_source_cannot_authorize_private_reply() {
    let (db, scope) = fixture("foreign-scope");
    let group = ConversationScope {
        target_type: "group".into(),
        ..scope.clone()
    };
    let foreign_id = save_incoming(&db, &group, 1);
    let (client, sent, task) = server(db.clone(), scope, false).await;
    let draft = draft(foreign_id);
    reply_workflow::save_plan_draft(&db, &client, &draft, vec![], 0).unwrap();
    assert!(
        reply_workflow::approve_draft(&db, &client, &draft.id, None, None)
            .await
            .is_err()
    );
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(db.get_pending_drafts().unwrap().len(), 1);
    task.abort();
}

fn persist_task(db: &Database, task: &crate::services::workflows::tasks::Task) {
    db.with_connection(|connection| {
        let run_id=task.active_run.as_ref().filter(|run|run.state=="running").map(|run|run.id.as_str());
        let count=connection.execute("UPDATE assistant_tasks SET revision=?2,state=?3,run_id=?4,lease_until=0,task_json=?5 WHERE task_id=?1",rusqlite::params![task.id,task.revision,task.state,run_id,serde_json::to_string(task).unwrap()])?;
        assert_eq!(count,1);Ok(())
    }).unwrap();
}

async fn completed_task(
    db: Arc<Database>,
    client: Arc<OneBotClient>,
    ai: Arc<crate::services::ai::AiService>,
) -> crate::services::workflows::tasks::Task {
    use crate::services::workflows::tasks::{self, Task, TaskRun};
    let created=tasks::manage(db.clone(),client,ai,json!({"operation":"create","targetType":"private","targetId":"20001","instruction":"基于当前会话整理候选回复"})).await.unwrap();
    let mut task: Task = serde_json::from_value(created["task"].clone()).unwrap();
    task.revision += 1;
    task.state = "completed".into();
    task.session.model = "completed-task-model".into();
    task.candidate_reply = Some(
        serde_json::to_value(
            ReplyPlan::from_bubble_texts(&["已整理".into(), "[CQ:face,id=324]".into()]).unwrap(),
        )
        .unwrap(),
    );
    task.active_run = Some(TaskRun {
        id: "completed_run".into(),
        sequence: 1,
        state: "completed".into(),
        input_revision: 1,
        persona_revision: 0,
        context_snapshot: vec![],
        started_at: 1000,
        finished_at: Some(2000),
        model_calls: 1,
        max_model_calls: 6,
        max_duration_seconds: 90,
    });
    persist_task(&db, &task);
    task
}

fn task_ai(db: &Database) -> Arc<crate::services::ai::AiService> {
    let config = crate::services::ai::AiRuntimeConfig::from_app_config(
        Some(
            &json!({"activeProvider":"custom","baseUrl":"http://127.0.0.1:9/v1","model":"currently-configured-model"}),
        ),
        "",
    );
    let path = db
        .with_connection(|connection| {
            connection.query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        })
        .unwrap();
    Arc::new(crate::services::ai::AiService::for_account(
        config,
        std::path::Path::new(&path).parent().unwrap().to_path_buf(),
    ))
}

#[tokio::test]
async fn task_candidate_export_is_idempotent_and_continuing_task_revokes_old_draft() {
    let (db, scope) = fixture("task-export");
    let (client, sent, server) = server(db.clone(), scope, false).await;
    let client = Arc::new(client);
    let ai = task_ai(&db);
    let mut task = completed_task(db.clone(), client.clone(), ai.clone()).await;
    let first = reply_workflow::create_task_draft(&db, &client, &ai, &task).unwrap();
    let second = reply_workflow::create_task_draft(&db, &client, &ai, &task).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.model_used, "completed-task-model");
    assert_eq!(first.reply_plan.as_ref().unwrap().bubbles.len(), 2);
    assert_eq!(db.get_pending_drafts().unwrap().len(), 1);
    task.revision += 1;
    task.state = "running".into();
    let active = task.active_run.as_mut().unwrap();
    active.id = "continued_run".into();
    active.state = "running".into();
    active.sequence += 1;
    active.finished_at = None;
    persist_task(&db, &task);
    let error = reply_workflow::approve_draft(&db, &client, &first.id, None, None)
        .await
        .unwrap_err();
    assert!(error.contains("原任务已继续"));
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(db.get_pending_drafts().unwrap().len(), 1);
    server.abort();
}

#[tokio::test]
async fn completed_task_candidate_from_another_account_cannot_create_a_draft() {
    let (db, scope) = fixture("task-foreign-account");
    let (client, sent, server) = server(db.clone(), scope, false).await;
    let client = Arc::new(client);
    let ai = task_ai(&db);
    let mut task = completed_task(db.clone(), client.clone(), ai.clone()).await;
    task.account_uin = "10002".into();
    assert!(reply_workflow::create_task_draft(&db, &client, &ai, &task).is_err());
    assert!(db.get_pending_drafts().unwrap().is_empty());
    assert!(sent.lock().unwrap().is_empty());
    server.abort();
}
