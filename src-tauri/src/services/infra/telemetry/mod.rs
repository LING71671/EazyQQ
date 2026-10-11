//! Opt-in Sentry diagnostics shared by desktop, CLI, workers and installer helpers.
mod privacy;
use std::sync::{atomic::{AtomicBool, Ordering}, OnceLock};

static ENABLED: AtomicBool = AtomicBool::new(false);
static COMPONENT: OnceLock<&'static str> = OnceLock::new();

pub fn release() -> String { option_env!("EAZYQQ_SENTRY_RELEASE").map(str::to_owned).unwrap_or_else(|| format!("eazyqq@{}", env!("CARGO_PKG_VERSION"))) }

pub fn initialize(component: &'static str) -> Option<sentry::ClientInitGuard> {
    let _ = COMPONENT.set(component);
    if cfg!(test) || std::env::var("EAZYQQ_TELEMETRY").as_deref() != Ok("1") { return None; }
    initialize_from_dsn(component, &scoped_dsn()?)
}

fn scoped_dsn() -> Option<String> { std::env::var("EAZYQQ_SENTRY_DSN").ok().or_else(|| option_env!("EAZYQQ_SENTRY_DSN").map(str::to_owned)).filter(|value| !value.trim().is_empty()) }

pub fn helper_dsn() -> Option<String> { ENABLED.load(Ordering::Relaxed).then(scoped_dsn).flatten() }

pub fn initialize_from_dsn(component: &'static str, dsn: &str) -> Option<sentry::ClientInitGuard> {
    if cfg!(test) || ENABLED.load(Ordering::Relaxed) { return None; }
    let _ = COMPONENT.set(component);
    let dsn = dsn.parse::<sentry::types::Dsn>().ok()?;
    let mut options = sentry::ClientOptions::new().release(release()).environment("development")
        .send_default_pii(false).server_name("eazyqq").sample_rate(1.0).traces_sample_rate(1.0).attach_stacktrace(true)
        .shutdown_timeout(std::time::Duration::from_secs(2))
        .before_breadcrumb(|_| None)
        .before_send(|event| serde_json::to_value(event).ok().and_then(privacy::sanitize).and_then(|event| serde_json::from_value(event).ok()))
        .before_send_log(move |mut log| {
            log.body = "EazyQQ structured runtime log".into();
            log.attributes.retain(|key, _| ["code.module.name", "code.line.number", "sentry.origin"].contains(&key.as_str()));
            log.attributes.insert("component".into(), sentry::protocol::LogAttribute(serde_json::Value::from(component)));
            log.attributes.insert("release".into(), sentry::protocol::LogAttribute(serde_json::Value::from(release())));
            Some(log)
        });
    options.dsn = Some(dsn);
    let guard = sentry::init(options);
    ENABLED.store(guard.is_enabled(), Ordering::Relaxed);
    sentry::configure_scope(|scope| scope.set_tag("component", component));
    Some(guard)
}

pub fn status() -> serde_json::Value {
    serde_json::json!({"enabled":ENABLED.load(Ordering::Relaxed),"configured":scoped_dsn().is_some(),"component":COMPONENT.get().copied().unwrap_or("backend"),"release":release()})
}

pub fn capture_failure(operation: &str, operation_id: Option<&str>, code: Option<i32>) -> Option<String> {
    if !ENABLED.load(Ordering::Relaxed) { return None; }
    Some(sentry::with_scope(|scope| {
        scope.set_tag("operation", operation);
        if let Some(id) = operation_id { scope.set_tag("operation_id", id); }
        if let Some(code) = code { scope.set_tag("error_code", code.to_string()); }
    }, || sentry::capture_message("EazyQQ operation failed", sentry::Level::Error)).simple().to_string())
}

pub fn test_event() -> Result<serde_json::Value, String> {
    let event_id = capture_failure("telemetry.test", None, None).ok_or("诊断上报未启用或DSN无效")?;
    let flushed = flush();
    Ok(serde_json::json!({"eventId":event_id,"flushed":flushed,"release":release()}))
}

pub fn flush() -> bool { sentry::Hub::current().client().is_some_and(|client| client.flush(Some(std::time::Duration::from_secs(5)))) }
