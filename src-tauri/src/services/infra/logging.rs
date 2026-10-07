//! Persistent file logging + panic capture.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

pub fn workspace_root() -> PathBuf {
    if let Some(root) = workspace_root_from_exe() {
        return root;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

/// Load key-value pairs from `.env` in the workspace root or current directory
/// into process environment variables, without overwriting existing environment variables.
pub fn load_dotenv() {
    let root = workspace_root();
    let candidates = [root.join(".env"), PathBuf::from(".env")];
    for path in &candidates {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    if !key.is_empty() && std::env::var(key).is_err() {
                        std::env::set_var(key, val);
                    }
                }
            }
            break;
        }
    }
}

fn workspace_root_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;

    let mut cur = dir;
    while let Some(parent) = cur.parent() {
        if parent
            .file_name()
            .map(|n| n.to_string_lossy() == "src-tauri")
            .unwrap_or(false)
        {
            return Some(parent.parent().unwrap_or(parent).to_path_buf());
        }
        cur = parent;
    }

    if let Some(parent) = dir.parent() {
        if parent.join("napcat").exists() || parent.join("EazyQQ_Data").exists() {
            return Some(parent.to_path_buf());
        }
    }

    Some(dir.to_path_buf())
}

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

fn timestamp_slug() -> String {
    let secs = (now_ms() / 1000) as i64 + 8 * 3600;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

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

pub fn init(mirror_stdout: bool) -> Option<PathBuf> {
    let dir = log_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[logging] cannot create log dir {:?}: {}", dir, e);
        return None;
    }

    let log_path = active_log_path();

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

pub fn tail(lines: usize) -> io::Result<String> {
    let path = active_log_path();
    let content = fs::read_to_string(&path)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

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

pub fn log_paths() {
    tracing::info!("workspace root : {}", workspace_root().display());
    tracing::info!("data dir       : {}", data_dir().display());
    tracing::info!("log dir        : {}", log_dir().display());
}

pub fn ensure_dir(path: &Path) -> bool {
    match fs::create_dir_all(path) {
        Ok(()) => true,
        Err(e) => {
            tracing::error!("cannot create directory {}: {}", path.display(), e);
            false
        }
    }
}
