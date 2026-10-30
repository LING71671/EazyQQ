use super::*;
use crate::services::ai::{AiRuntimeConfig, AiService};

fn root() -> PathBuf {
    crate::services::logging::workspace_root()
        .join(crate::services::workflows::tasks::new_id("request_cache").unwrap())
}
fn id() -> String {
    crate::services::workflows::tasks::new_id("request").unwrap()
}

#[test]
fn dropping_a_request_removes_only_its_own_runtime() {
    let root = root();
    let persistent = root.join("ai-runtime/tasks/task_1");
    std::fs::create_dir_all(&persistent).unwrap();
    std::fs::write(persistent.join("history.json"), "keep task history").unwrap();
    let ai = AiService::for_account(AiRuntimeConfig::default(), root.clone());
    let request = ai.for_request(&id()).unwrap();
    let path = request.runtime_dir().to_path_buf();
    std::fs::write(path.join("private-result.json"), "private response").unwrap();
    assert_ne!(path, persistent);
    assert!(path.exists());
    drop(request);
    assert!(!path.exists());
    assert!(persistent.join("history.json").is_file());
    assert!(ai.runtime_dir().is_dir());
}

#[test]
fn requests_require_fresh_identifiers_and_do_not_reuse_a_live_directory() {
    let root = root();
    let ai = AiService::for_account(AiRuntimeConfig::default(), root);
    for bad in [
        "../user",
        "simple",
        ".._01234567890123456789012345678901",
        "request_/01234567890123456789012345678901",
    ] {
        assert!(ai.for_request(bad).is_err());
    }
    let id = id();
    let first = ai.for_request(&id).unwrap();
    assert!(ai.for_request(&id).is_err());
    let lock = first.runtime_dir().join("request.lock");
    assert!(FileLock::try_acquire(&lock).is_err());
}

#[test]
fn admission_limit_preserves_other_requests_without_pruning_them() {
    let root = root();
    let ai = AiService::for_account(AiRuntimeConfig::default(), root);
    let requests = (0..MAX_REQUESTS)
        .map(|_| ai.for_request(&id()).unwrap())
        .collect::<Vec<_>>();
    assert!(ai.for_request(&id()).is_err());
    assert!(requests
        .iter()
        .all(|request| request.runtime_dir().exists()));
    drop(requests);
    assert!(ai.for_request(&id()).is_ok());
}

#[test]
fn oversized_request_caches_are_rejected_then_removed() {
    let root = root();
    let guard = RequestRuntime::create(&root, &id()).unwrap();
    let path = guard.path().to_path_buf();
    let file = std::fs::File::create(path.join("oversized.bin")).unwrap();
    file.set_len(MAX_REQUEST_BYTES + 1).unwrap();
    drop(file);
    assert!(guard.check_budget().is_err());
    drop(guard);
    assert!(!path.exists());
}

#[cfg(windows)]
#[test]
fn reparse_points_cannot_route_a_request_or_cleanup_into_other_files() {
    use std::os::windows::fs::symlink_dir;
    let root = root();
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "keep user file").unwrap();
    let account = root.join("account");
    std::fs::create_dir_all(&account).unwrap();
    symlink_dir(&outside, account.join("ai-runtime")).unwrap();
    let ai = AiService::for_account(AiRuntimeConfig::default(), account);
    assert!(ai.for_request(&id()).is_err());
    assert!(outside.join("keep.txt").is_file());
}

#[test]
fn transient_inference_keeps_the_bound_accounts_style_pack() {
    use crate::services::ai::style_pack::{
        self, LineBreaks, Punctuation, SentenceLength, StylePack, StyleRules, Tone,
    };
    let account = root().join("10001");
    std::fs::create_dir_all(&account).unwrap();
    let pack = StylePack {
        schema_version: 1,
        id: "fixture-style".into(),
        name: "合成风格".into(),
        version: "1.0.0".into(),
        rules: StyleRules {
            sentence_length: SentenceLength::Short,
            punctuation: Punctuation::Minimal,
            tone: Tone::Neutral,
            line_breaks: LineBreaks::Natural,
            preferred_phrases: vec!["style-mark".into()],
            avoid_phrases: Vec::new(),
            qq_face_ids: Vec::new(),
        },
        statistics: None,
        profile: None,
    };
    style_pack::import_for_account(&account, &serde_json::to_vec(&pack).unwrap()).unwrap();
    let ai = AiService::for_account(AiRuntimeConfig::default(), account.clone());
    let request = ai.for_request(&id()).unwrap();
    assert!(request
        .task_system_prompt()
        .unwrap()
        .contains("style-mark"));
    request.release_request_runtime().unwrap();
    assert_eq!(
        style_pack::read_for_account(&account)
            .unwrap()
            .unwrap()
            .pack
            .id,
        "fixture-style"
    );
    let first_session = ai.for_task_session("same_task", "session_first").unwrap();
    let second_session = ai.for_task_session("same_task", "session_second").unwrap();
    assert_ne!(first_session.runtime_dir(), second_session.runtime_dir());
    assert!(first_session
        .task_system_prompt()
        .unwrap()
        .contains("style-mark"));
    assert!(second_session
        .task_system_prompt()
        .unwrap()
        .contains("style-mark"));
    assert!(first_session
        .runtime_dir()
        .starts_with(ai.for_task("same_task").unwrap().runtime_dir()));
    assert!(ai
        .for_task_session("same_task", "../other_session")
        .is_err());
    assert!(ai
        .for_task("persistent_task")
        .unwrap()
        .release_request_runtime()
        .is_err());
}

#[tokio::test]
async fn cancelling_an_owned_future_releases_its_request_cache() {
    let ai = AiService::for_account(AiRuntimeConfig::default(), root());
    let request = ai.for_request(&id()).unwrap();
    let path = request.runtime_dir().to_path_buf();
    let running = tokio::spawn(async move {
        let _request = request;
        std::future::pending::<()>().await;
    });
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    assert!(!path.exists());
}

#[cfg(windows)]
#[test]
fn cleanup_refuses_a_link_inserted_into_its_owned_directory() {
    use std::os::windows::fs::symlink_dir;
    let root = root();
    let outside = root.join("other-files");
    std::fs::create_dir_all(&outside).unwrap();
    let keep = outside.join("keep.txt");
    std::fs::write(&keep, "keep user file").unwrap();
    let guard = RequestRuntime::create(&root.join("account"), &id()).unwrap();
    let path = guard.path().to_path_buf();
    symlink_dir(&outside, path.join("injected-link")).unwrap();
    assert!(guard.release().is_err());
    assert!(keep.is_file());
    assert!(path.exists());
}
