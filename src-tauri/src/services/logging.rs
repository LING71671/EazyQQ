//! Persistent file logging + panic capture.
//!
//! Before this module existed the backend had no durable logs at all: a black window
//! or a hung process left no trace to diagnose. Everything now lands in
//! `<workspace>/EazyQQ_Data/logs/`, and every panic writes a standalone crash report
//! with a forced backtrace.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

/// Rotate the active log once it grows past this size.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Resolve the workspace root the same way `lib.rs` does, so the CLI and the GUI
/// always agree on where logs and the SQLite database live.
pub fn workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

/// Data directory for the account this process is serving.
///
/// Every private artefact hangs off this one path - the SQLite database, the log file
/// (which contains message text verbatim), downloaded group files and diagnostics - so
/// scoping it by account is what keeps one QQ account's data away from another's. The
/// active account is resolved once at startup by `accounts`; see that module for the
/// layout.
pub fn data_dir() -> PathBuf {
    crate::services::accounts::active_data_dir()
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn active_log_path() -> PathBuf {
    log_dir().join("eazyqq.log")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// `2026-09-21_23-40-12` in local-ish (UTC+8) terms, used for file names.
fn timestamp_slug() -> String {
    let secs = (now_ms() / 1000) as i64 + 8 * 3600;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Civil date from days since the Unix epoch (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", y, mo, d, h, m, s)
}

/// Append-only writer that mirrors everything to stdout as well, so the CLI is usable
/// interactively while the same lines land in the log file.
#[derive(Clone)]
struct TeeWriter {
    file: Arc<Mutex<File>>,
    mirror_stdout: bool,
}

pub struct TeeGuard {
    file: Arc<Mutex<File>>,
    mirror_stdout: bool,
}

impl Write for TeeGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(buf);
            let _ = f.flush();
        }
        if self.mirror_stdout {
            let _ = io::stdout().write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
        if self.mirror_stdout {
            let _ = io::stdout().flush();
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for TeeWriter {
    type Writer = TeeGuard;

    fn make_writer(&'a self) -> Self::Writer {
        TeeGuard {
            file: self.file.clone(),
            mirror_stdout: self.mirror_stdout,
        }
    }
}

/// Initialise the global tracing subscriber.
///
/// Safe to call more than once (later calls are ignored) so both `main.rs` and the CLI
/// binary can call it unconditionally.
pub fn init(mirror_stdout: bool) -> Option<PathBuf> {
    let dir = log_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[logging] cannot create log dir {:?}: {}", dir, e);
        return None;
    }

    let log_path = active_log_path();

    // Rotate before opening for append.
    if let Ok(meta) = fs::metadata(&log_path) {
        if meta.len() > MAX_LOG_BYTES {
            let rotated = dir.join(format!("eazyqq-{}.log", timestamp_slug()));
            let _ = fs::rename(&log_path, rotated);
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()?;

    let writer = TeeWriter {
        file: Arc::new(Mutex::new(file)),
        mirror_stdout,
    };

    let level = std::env::var("EAZYQQ_LOG")
        .ok()
        .and_then(|v| v.parse::<Level>().ok())
        .unwrap_or(Level::INFO);

    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_max_level(level)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .try_init();

    install_panic_hook();

    tracing::info!(
        "=== EazyQQ logging initialised (level={}, file={}) ===",
        level,
        log_path.display()
    );

    Some(log_path)
}

/// Write a standalone crash report for every panic, with a forced backtrace.
pub fn install_panic_hook() {
    use std::sync::Once;
    static HOOK: Once = Once::new();

    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string();
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "<unknown location>".to_string());
            let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            let backtrace = std::backtrace::Backtrace::force_capture();

            let report = format!(
                "EazyQQ crash report\n\
                 time       : {}\n\
                 thread     : {}\n\
                 location   : {}\n\
                 payload    : {}\n\
                 version    : {}\n\
                 \n--- backtrace ---\n{}\n",
                timestamp_slug(),
                thread,
                location,
                payload,
                env!("CARGO_PKG_VERSION"),
                backtrace
            );

            let dir = log_dir();
            let _ = fs::create_dir_all(&dir);
            let crash_path = dir.join(format!("crash-{}.log", timestamp_slug()));
            let _ = fs::write(&crash_path, &report);
            let _ = fs::write(dir.join("last-crash.log"), &report);

            tracing::error!(
                "PANIC in thread '{}' at {}: {} (report: {})",
                thread,
                location,
                payload,
                crash_path.display()
            );

            previous(info);
        }));
    });
}

/// Read the tail of the active log file (used by `eazyqq-cli log-tail`).
pub fn tail(lines: usize) -> io::Result<String> {
    let path = active_log_path();
    let content = fs::read_to_string(&path)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

/// Report the log directory and how many crash reports exist, for health checks.
pub fn diagnostics() -> (PathBuf, usize, Option<PathBuf>) {
    let dir = log_dir();
    let count = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("crash-")
                })
                .count()
        })
        .unwrap_or(0);
    let last = dir.join("last-crash.log");
    let last = if last.exists() { Some(last) } else { None };
    (dir, count, last)
}

/// Convenience for writing a startup banner that includes the resolved paths.
pub fn log_paths() {
    tracing::info!("workspace root : {}", workspace_root().display());
    tracing::info!("data dir       : {}", data_dir().display());
    tracing::info!("log dir        : {}", log_dir().display());
}

/// Ensure the given directory exists, logging (not swallowing) any failure.
pub fn ensure_dir(path: &Path) -> bool {
    match fs::create_dir_all(path) {
        Ok(()) => true,
        Err(e) => {
            tracing::error!("cannot create directory {}: {}", path.display(), e);
            false
        }
    }
}
