use super::*;
use crate::models::MessageItemDto;
use crate::services::protocol::context::{self, ConversationScope};
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn database() -> Database {
    let db = Database::init(":memory:").unwrap();
    store::initialize(&db).unwrap();
    db
}

fn config() -> AiRuntimeConfig {
    AiRuntimeConfig {
        provider: "custom".into(),
        base_url: "http://127.0.0.1:9/v1".into(),
        model: "fixture-model".into(),
        api_key: String::new(),
        ..Default::default()
    }
}

fn fixture_task(db: &Database, allow_todos: bool) -> Task {
    let config = config();
    let task = Task {
        id: new_id("task").unwrap(),
        account_uin: "10001".into(),
        target_id: "20001".into(),
        target_type: "private".into(),
        title: "Fixture task".into(),
        instruction: "Find the original decision".into(),
        persona: String::new(),
        allow_todos,
        selected_file_ids: Vec::new(),
        revision: 1,
        state: "ready".into(),
        session: TaskSession {
            id: new_id("session").unwrap(),
            generation: 1,
            provider: config.provider.clone(),
            model: config.model.clone(),
        },
        messages: Vec::new(),
        steps: Vec::new(),
        todos: Vec::new(),
        result: None,
        candidate_reply: None,
        error: None,
        active_run: None,
        created_at: now_ms(),
        updated_at: now_ms(),
    };
    store::insert(db, &task, &fingerprint(&config)).unwrap();
    task
}

#[test]
fn tasks_reject_other_accounts_and_preserve_separate_transcripts() {
    let db = database();
    let first = fixture_task(&db, false);
    let second = fixture_task(&db, false);
    assert!(store::get(&db, "10002", &first.id).is_err());
    let running = store::claim(
        &db,
        "10001",
        &first.id,
        "first private detail",
        Some(1),
        &config(),
    )
    .unwrap();
    assert_eq!(running.messages[0].content, "first private detail");
    assert!(store::get(&db, "10001", &second.id)
        .unwrap()
        .messages
        .is_empty());
    assert!(store::claim(&db, "10001", &first.id, "second request", None, &config()).is_err());
}

#[test]
fn optimistic_revision_cancel_and_stale_completion_are_atomic() {
    let db = database();
    let task = fixture_task(&db, false);
    assert!(store::claim(&db, "10001", &task.id, "", Some(9), &config()).is_err());
    let first = store::claim(&db, "10001", &task.id, "", Some(1), &config()).unwrap();
    let old_run = first.active_run.unwrap().id;
    let cancelled = store::cancel(&db, "10001", &task.id).unwrap();
    assert_eq!(cancelled.state, "cancelled");
    let next = store::claim(
        &db,
        "10001",
        &task.id,
        "continue",
        Some(cancelled.revision),
        &config(),
    )
    .unwrap();
    assert_eq!(next.active_run.as_ref().unwrap().sequence, 2);
    assert!(store::update_run(&db, "10001", &task.id, &old_run, |task| {
        task.result = Some("stale".into());
        Ok(())
    })
    .is_err());
    assert!(store::get(&db, "10001", &task.id).unwrap().result.is_none());
}

#[test]
fn auth_endpoint_and_model_changes_rotate_the_task_session() {
    let db = database();
    let task = fixture_task(&db, false);
    let original_session = task.session.id.clone();
    let mut changed = config();
    changed.api_key = "fixture-private-key".into();
    let run = store::claim(&db, "10001", &task.id, "", None, &changed).unwrap();
    assert_eq!(run.session.generation, 2);
    assert_ne!(run.session.id, original_session);
    assert!(!serde_json::to_string(&run)
        .unwrap()
        .contains("fixture-private-key"));
    store::cancel(&db, "10001", &task.id).unwrap();
    changed.model = "fixture-next-model".into();
    changed.base_url = "http://127.0.0.1:8/v1".into();
    let run = store::claim(&db, "10001", &task.id, "", None, &changed).unwrap();
    assert_eq!(run.session.generation, 3);
}

#[test]
fn expired_runs_recover_as_interrupted_without_claiming_success() {
    let db = database();
    let task = fixture_task(&db, false);
    let run = store::claim(&db, "10001", &task.id, "", None, &config()).unwrap();
    store::recover_expired(&db, "10001", now_ms() + 100_000).unwrap();
    let interrupted = store::get(&db, "10001", &task.id).unwrap();
    assert_eq!(interrupted.state, "interrupted");
    assert!(interrupted.result.is_none());
    assert!(
        store::update_run(&db, "10001", &task.id, &run.active_run.unwrap().id, |_| Ok(
            ()
        ))
        .is_err()
    );
}

#[test]
fn independent_database_connections_exclude_a_second_process_claim() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-runtime")
        .join(std::process::id().to_string())
        .join(new_id("task_claim").unwrap());
    let path = root.join("tasks.db");
    let first = Database::init(&path).unwrap();
    let second = Database::init(&path).unwrap();
    store::initialize(&first).unwrap();
    let task = fixture_task(&first, false);
    store::claim(&first, "10001", &task.id, "GUI", None, &config()).unwrap();
    assert!(store::claim(&second, "10001", &task.id, "CLI", None, &config()).is_err());
    assert!(store::delete(&second, "10001", &task.id).is_err());
}

#[test]
fn tool_permissions_and_scope_cannot_be_changed_by_model_output() {
    let db = database();
    let task = fixture_task(&db, false);
    for call in [
        json!({"action":"search_history","targetId":"another"}),
        json!({"action":"list_group_files"}),
        json!({"action":"set_todos","todos":[{"text":"unauthorized"}]}),
        json!({"action":"shell","command":"anything"}),
        json!({"action":"search_history","path":"C:/private"}),
    ] {
        assert!(tools::execute(&db, &task, &call).is_err());
    }
    let allowed = fixture_task(&db, true);
    let result = tools::execute(
        &db,
        &allowed,
        &json!({"action":"set_todos","todos":[{"text":"Check the cited decision","done":false}]}),
    )
    .unwrap();
    assert_eq!(result.todos.unwrap()[0].text, "Check the cited decision");
    assert_eq!(result.output["externalSync"], false);
}

#[test]
fn history_tool_requires_exact_verified_scope_and_keeps_group_authors_distinct() {
    let db = database();
    let mut task = fixture_task(&db, false);
    task.target_type = "group".into();
    let scope = ConversationScope {
        account_uin: task.account_uin.clone(),
        target_type: task.target_type.clone(),
        target_id: task.target_id.clone(),
    };
    for (id, sender, name) in [
        ("a", "10001", "Same name"),
        ("b", "30001", "Same name"),
        ("c", "30002", "Same name"),
    ] {
        let raw = json!({"message_id":id,"message_type":"group","group_id":scope.target_id,"user_id":sender,"sender":{"user_id":sender,"nickname":name},"message":"decision detail","time":1});
        let message = context::message_from_protocol(&scope, &raw, 1).unwrap();
        context::store_message(
            &db,
            &scope,
            &message,
            &context::source_record(&scope, "event", &raw),
            false,
        )
        .unwrap();
    }
    let unknown = MessageItemDto {
        id: "unverified".into(),
        target_id: task.target_id.clone(),
        sender_id: "30003".into(),
        sender_name: "Same name".into(),
        content: "decision unverified".into(),
        is_from_me: false,
        ai_reply_status: "none".into(),
        timestamp: 5,
    };
    db.save_message(&unknown).unwrap();
    let private_scope = ConversationScope {
        target_type: "private".into(),
        ..scope.clone()
    };
    let raw = json!({"message_id":"private","message_type":"private","user_id":"20001","message":"decision private"});
    let message = context::message_from_protocol(&private_scope, &raw, 1).unwrap();
    context::store_message(
        &db,
        &private_scope,
        &message,
        &context::source_record(&private_scope, "event", &raw),
        false,
    )
    .unwrap();
    let result = tools::execute(
        &db,
        &task,
        &json!({"action":"search_history","query":"decision"}),
    )
    .unwrap();
    let matches = result.output["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(result.sources.len(), 3);
    let keys = matches
        .iter()
        .map(|message| message["memberKey"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(keys.len(), 3);
    assert_eq!(
        matches
            .iter()
            .filter(|message| message["relation"] == "account_owner")
            .count(),
        1
    );
}

#[test]
fn selected_file_tool_reads_only_explicit_owned_group_cache() {
    let db = database();
    let mut task = fixture_task(&db, false);
    task.target_type = "group".into();
    task.selected_file_ids = vec!["file-a".into()];
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-runtime")
        .join(std::process::id().to_string())
        .join(new_id("task_material").unwrap());
    let group = root.join("group_files").join(&task.target_id);
    std::fs::create_dir_all(&group).unwrap();
    let path = group.join("decision.txt");
    std::fs::write(&path, "Source decision: review Friday").unwrap();
    let mut record = crate::services::db::GroupFileRecord {
        file_id: "file-a".into(),
        group_id: task.target_id.clone(),
        file_name: "decision.txt".into(),
        file_size: 30,
        busid: 1,
        uploader_name: "Peer".into(),
        upload_time: 1,
        local_path: Some(path.to_string_lossy().into_owned()),
        download_status: "downloaded".into(),
        updated_at: 1,
    };
    db.upsert_group_file(&record).unwrap();
    let result = tools::read_selected_file(
        &db,
        &task,
        &json!({"action":"read_selected_file","fileId":"file-a"}),
        &root,
    )
    .unwrap();
    assert!(result.output["text"]
        .as_str()
        .unwrap()
        .contains("review Friday"));
    assert_eq!(result.sources[0]["groupId"], task.target_id);
    assert!(tools::read_selected_file(
        &db,
        &task,
        &json!({"action":"read_selected_file","fileId":"not-selected"}),
        &root
    )
    .is_err());
    let outside = root.join("outside.txt");
    std::fs::write(&outside, "must not read").unwrap();
    record.local_path = Some(outside.to_string_lossy().into_owned());
    db.upsert_group_file(&record).unwrap();
    assert!(tools::read_selected_file(
        &db,
        &task,
        &json!({"action":"read_selected_file","fileId":"file-a"}),
        &root
    )
    .is_err());
    record.download_status = "remote".into();
    db.upsert_group_file(&record).unwrap();
    assert!(tools::read_selected_file(
        &db,
        &task,
        &json!({"action":"read_selected_file","fileId":"file-a"}),
        &root
    )
    .unwrap_err()
    .contains("尚未下载"));
}

async fn scripted_server(
    actions: Vec<Value>,
    delay_ms: u64,
) -> (String, Arc<Mutex<Vec<Value>>>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let captured = Arc::new(Mutex::new(Vec::new()));
    let requests = captured.clone();
    let handle = tokio::spawn(async move {
        for action in actions {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            let (header_end, content_length) = loop {
                let count = stream.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                buffer.extend_from_slice(&chunk[..count]);
                if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&buffer[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:").map(str::trim))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    break (end + 4, length);
                }
                assert!(buffer.len() < 1_000_000);
            };
            while buffer.len() < header_end + content_length {
                let count = stream.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                buffer.extend_from_slice(&chunk[..count]);
            }
            let body =
                serde_json::from_slice(&buffer[header_end..header_end + content_length]).unwrap();
            requests.lock().unwrap().push(body);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            let body = json!({"choices":[{"message":{"content":action.to_string()}}]}).to_string();
            let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    (endpoint, captured, handle)
}

fn mock_ai(endpoint: &str) -> Arc<AiService> {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-runtime")
        .join(std::process::id().to_string())
        .join(new_id("task_ai").unwrap());
    Arc::new(AiService::for_account(
        AiRuntimeConfig {
            base_url: endpoint.into(),
            ..config()
        },
        root,
    ))
}

#[tokio::test]
async fn bounded_agent_runs_tools_then_returns_candidate_without_sending() {
    let (endpoint,captured,server)=scripted_server(vec![json!({"action":"set_todos","todos":[{"text":"Review source"}]}),json!({"action":"complete","result":"Task local todo prepared","candidateReply":{"schemaVersion":1,"bubbles":[{"segments":[{"type":"text","text":"收到"}]},{"segments":[{"type":"face","faceKey":"427"}]}]}})],0).await;
    let db = Arc::new(database());
    let task = fixture_task(&db, true);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    let result = runtime::run(
        db.clone(),
        onebot,
        ai,
        "10001",
        &task.id,
        &json!({"instruction":"prepare only"}),
    )
    .await
    .unwrap();
    assert_eq!(result.state, "completed");
    assert_eq!(result.todos.len(), 1);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.active_run.unwrap().model_calls, 2);
    assert_eq!(
        result.candidate_reply.unwrap()["bubbles"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    server.await.unwrap();
    assert_eq!(captured.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn task_continuation_retains_own_history_without_leaking_other_tasks() {
    let (endpoint, captured, server) = scripted_server(
        vec![
            json!({"action":"complete","result":"first scoped answer"}),
            json!({"action":"complete","result":"continued scoped answer"}),
            json!({"action":"complete","result":"other task answer"}),
        ],
        0,
    )
    .await;
    let db = Arc::new(database());
    let first = fixture_task(&db, false);
    let second = fixture_task(&db, false);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    runtime::run(
        db.clone(),
        onebot.clone(),
        ai.clone(),
        "10001",
        &first.id,
        &json!({"instruction":"unique-task-one-fact"}),
    )
    .await
    .unwrap();
    runtime::run(
        db.clone(),
        onebot.clone(),
        ai.clone(),
        "10001",
        &first.id,
        &json!({"instruction":"continue original"}),
    )
    .await
    .unwrap();
    runtime::run(
        db.clone(),
        onebot,
        ai,
        "10001",
        &second.id,
        &json!({"instruction":"unique-task-two-fact"}),
    )
    .await
    .unwrap();
    server.await.unwrap();
    let requests = captured.lock().unwrap();
    assert!(requests[1].to_string().contains("unique-task-one-fact"));
    assert!(requests[1].to_string().contains("first scoped answer"));
    assert!(!requests[2].to_string().contains("unique-task-one-fact"));
    assert!(!requests[2].to_string().contains("first scoped answer"));
}

#[tokio::test]
async fn cancellation_during_inference_cannot_commit_a_late_success() {
    let (endpoint, captured, server) = scripted_server(
        vec![json!({"action":"complete","result":"must not commit"})],
        1_000,
    )
    .await;
    let db = Arc::new(database());
    let task = fixture_task(&db, false);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    let run_db = db.clone();
    let id = task.id.clone();
    let running =
        tokio::spawn(
            async move { runtime::run(run_db, onebot, ai, "10001", &id, &json!({})).await },
        );
    for _ in 0..100 {
        if !captured.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!captured.lock().unwrap().is_empty());
    store::cancel(&db, "10001", &task.id).unwrap();
    let result = running.await.unwrap().unwrap();
    assert_eq!(result.state, "cancelled");
    assert!(result.result.is_none());
    server.abort();
}

#[tokio::test]
async fn incoming_message_invalidates_a_running_task_candidate() {
    let (endpoint, captured, server) = scripted_server(
        vec![json!({"action":"complete","result":"obsolete reply"})],
        1_000,
    )
    .await;
    let db = Arc::new(database());
    let task = fixture_task(&db, false);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    let run_db = db.clone();
    let id = task.id.clone();
    let running =
        tokio::spawn(
            async move { runtime::run(run_db, onebot, ai, "10001", &id, &json!({})).await },
        );
    for _ in 0..100 {
        if !captured.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!captured.lock().unwrap().is_empty());
    let scope = ConversationScope {
        account_uin: task.account_uin.clone(),
        target_id: task.target_id.clone(),
        target_type: task.target_type.clone(),
    };
    let raw = json!({"message_id":"new","message_type":"private","user_id":"20001","message":"new request supersedes previous input","time":2});
    let message = context::message_from_protocol(&scope, &raw, 2).unwrap();
    context::store_message(
        &db,
        &scope,
        &message,
        &context::source_record(&scope, "event", &raw),
        false,
    )
    .unwrap();
    let result = running.await.unwrap().unwrap();
    assert_eq!(result.state, "failed");
    assert!(result.result.is_none());
    assert!(result.error.unwrap().contains("新消息"));
    server.abort();
}

#[tokio::test]
async fn authentication_change_during_inference_prevents_stale_result_commit() {
    let (endpoint, captured, server) = scripted_server(
        vec![json!({"action":"complete","result":"old-auth result"})],
        1_000,
    )
    .await;
    let db = Arc::new(database());
    let task = fixture_task(&db, false);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    let run_db = db.clone();
    let run_ai = ai.clone();
    let id = task.id.clone();
    let running = tokio::spawn(async move {
        runtime::run(run_db, onebot, run_ai, "10001", &id, &json!({})).await
    });
    for _ in 0..100 {
        if !captured.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!captured.lock().unwrap().is_empty());
    let mut changed = ai.current();
    changed.api_key = "new-fixture-key".into();
    ai.reconfigure(changed);
    let result = running.await.unwrap().unwrap();
    assert_eq!(result.state, "failed");
    assert!(result.result.is_none());
    assert!(result.error.unwrap().contains("认证配置"));
    server.abort();
}

#[tokio::test]
async fn tool_loop_stops_at_the_model_call_budget() {
    let (endpoint, captured, server) = scripted_server(
        vec![json!({"action":"search_history","query":""}); MAX_MODEL_CALLS],
        0,
    )
    .await;
    let db = Arc::new(database());
    let task = fixture_task(&db, false);
    let ai = mock_ai(&endpoint);
    let onebot = Arc::new(OneBotClient::for_account(
        "http://127.0.0.1:1".into(),
        Some("10001".into()),
    ));
    let result = runtime::run(db, onebot, ai, "10001", &task.id, &json!({}))
        .await
        .unwrap();
    assert_eq!(result.state, "failed");
    assert!(result.error.unwrap().contains("6 次"));
    assert_eq!(result.steps.len(), MAX_MODEL_CALLS);
    server.await.unwrap();
    assert_eq!(captured.lock().unwrap().len(), MAX_MODEL_CALLS);
}
