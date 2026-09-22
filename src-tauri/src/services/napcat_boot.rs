//! Start (and restart) the NapCat protocol backend.
//!
//! `napcat.autoRestart` has been in the settings UI since the beginning and was never
//! read by anything, so the app could only ever report that the protocol side was down -
//! never do something about it. This is the missing half of that pair: the chain monitor
//! detects the break, and this module acts on it.
//!
//! Starting NapCat means reproducing what the shipped batch launcher does, because there
//! is no other supported entry point:
//!   1. write `loadNapCat.js`, which imports `napcat.mjs`
//!   2. export the NAPCAT_* variables
//!   3. run `NapCatWinBootMain.exe <QQ.exe> <NapCatWinBootHook.dll>`
//!
//! Restarting is rate limited on purpose. A backend that cannot start will fail every
//! time, and an unthrottled retry loop would spawn processes forever and bury the real
//! error in noise.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Wait at least this long between start attempts.
const MIN_RESTART_INTERVAL: Duration = Duration::from_secs(90);
/// Give up after this many consecutive failures and wait for a manual retry.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct BootOutcome {
    pub attempted: bool,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Default)]
struct BootState {
    last_attempt: Option<Instant>,
    consecutive_failures: u32,
}

fn state() -> &'static Mutex<BootState> {
    static S: std::sync::OnceLock<Mutex<BootState>> = std::sync::OnceLock::new();
    S.get_or_init(|| Mutex::new(BootState::default()))
}

/// The QQ path NapCat should launch, read from `config/qq_path.txt`.
///
/// Written by NapCat's own launcher; we only read it. No fallback path is invented - a
/// guessed path would launch some other installation, or nothing.
pub fn configured_qq_path(napcat_dir: &Path) -> Result<PathBuf, String> {
    let cfg = napcat_dir.join("config").join("qq_path.txt");
    let raw = std::fs::read_to_string(&cfg)
        .map_err(|e| format!("读取 {} 失败: {}", cfg.display(), e))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{} 为空，无法确定 QQ 路径", cfg.display()));
    }
    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(format!(
            "{} 指向的 QQ 不存在: {}（请在 NapCat 启动器中重新选择）",
            cfg.display(),
            path.display()
        ));
    }
    Ok(path)
}

/// Everything needed to start NapCat, resolved and checked up front.
struct BootFiles {
    launcher: PathBuf,
    hook_dll: PathBuf,
    main_mjs: PathBuf,
    load_js: PathBuf,
    patch_pkg: PathBuf,
    qq_path: PathBuf,
}

fn resolve(napcat_dir: &Path) -> Result<BootFiles, String> {
    let launcher = napcat_dir.join("NapCatWinBootMain.exe");
    let hook_dll = napcat_dir.join("NapCatWinBootHook.dll");
    let main_mjs = napcat_dir.join("napcat.mjs");
    let load_js = napcat_dir.join("loadNapCat.js");
    let patch_pkg = napcat_dir.join("qqnt.json");

    for (path, what) in [
        (&launcher, "NapCatWinBootMain.exe"),
        (&hook_dll, "NapCatWinBootHook.dll"),
        (&main_mjs, "napcat.mjs"),
    ] {
        if !path.exists() {
            return Err(format!(
                "NapCat 安装不完整：缺少 {}（{}）",
                what,
                path.display()
            ));
        }
    }

    Ok(BootFiles {
        launcher,
        hook_dll,
        main_mjs,
        load_js,
        patch_pkg,
        qq_path: configured_qq_path(napcat_dir)?,
    })
}

/// Should an automatic start be attempted right now?
///
/// Returns `Err(reason)` when it should not, so the caller can log a useful reason rather
/// than silently skipping.
fn may_attempt() -> Result<(), String> {
    let st = state().lock().map_err(|_| "状态锁不可用".to_string())?;
    if st.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
        return Err(format!(
            "已连续失败 {} 次，停止自动重启以避免循环（可手动启动 NapCat 后恢复）",
            st.consecutive_failures
        ));
    }
    if let Some(last) = st.last_attempt {
        let elapsed = last.elapsed();
        if elapsed < MIN_RESTART_INTERVAL {
            return Err(format!(
                "距上次尝试仅 {} 秒，需等待 {} 秒",
                elapsed.as_secs(),
                MIN_RESTART_INTERVAL.as_secs()
            ));
        }
    }
    Ok(())
}

fn note_attempt() {
    if let Ok(mut st) = state().lock() {
        st.last_attempt = Some(Instant::now());
    }
}

fn note_result(ok: bool) {
    if let Ok(mut st) = state().lock() {
        if ok {
            st.consecutive_failures = 0;
        } else {
            st.consecutive_failures += 1;
        }
    }
}

/// Reset the failure counter (called when the backend is observed healthy again).
pub fn note_healthy() {
    if let Ok(mut st) = state().lock() {
        st.consecutive_failures = 0;
    }
}

pub fn consecutive_failures() -> u32 {
    state().lock().map(|s| s.consecutive_failures).unwrap_or(0)
}

/// Launch NapCat. Returns immediately; readiness is confirmed later by the chain monitor.
pub fn start(napcat_dir: &Path) -> BootOutcome {
    if let Err(reason) = may_attempt() {
        return BootOutcome {
            attempted: false,
            ok: false,
            detail: reason,
        };
    }

    let files = match resolve(napcat_dir) {
        Ok(f) => f,
        Err(e) => {
            note_result(false);
            return BootOutcome {
                attempted: true,
                ok: false,
                detail: e,
            };
        }
    };

    note_attempt();

    // Step 1: the loader stub. NapCat's launcher converts backslashes for the file:// URL.
    let main_url = files.main_mjs.to_string_lossy().replace('\\', "/");
    let loader = format!("(async () => {{await import(\"file:///{}\")}})()", main_url);
    if let Err(e) = std::fs::write(&files.load_js, loader) {
        note_result(false);
        return BootOutcome {
            attempted: true,
            ok: false,
            detail: format!("写入 {} 失败: {}", files.load_js.display(), e),
        };
    }

    // Step 2 + 3: same variables the batch launcher exports, then run the bootstrapper.
    let spawn = Command::new(&files.launcher)
        .arg(&files.qq_path)
        .arg(&files.hook_dll)
        .current_dir(napcat_dir)
        .env("NAPCAT_PATCH_PACKAGE", &files.patch_pkg)
        .env("NAPCAT_LOAD_PATH", &files.load_js)
        .env("NAPCAT_INJECT_PATH", &files.hook_dll)
        .env("NAPCAT_LAUNCHER_PATH", &files.launcher)
        .env("NAPCAT_MAIN_PATH", &files.main_mjs)
        .env("QQ_PATH_CONFIG", napcat_dir.join("config").join("qq_path.txt"))
        .spawn();

    match spawn {
        Ok(child) => {
            tracing::info!(
                "napcat boot: launched {} (pid {}) with QQ {}",
                files.launcher.display(),
                child.id(),
                files.qq_path.display()
            );
            // The bootstrapper hands off to QQ and exits, so a successful spawn only means
            // "started", not "ready" - the chain monitor decides that.
            BootOutcome {
                attempted: true,
                ok: true,
                detail: format!("已拉起 NapCat（QQ: {}）", files.qq_path.display()),
            }
        }
        Err(e) => {
            note_result(false);
            BootOutcome {
                attempted: true,
                ok: false,
                detail: format!("启动 {} 失败: {}", files.launcher.display(), e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_install_reports_which_file_is_absent() {
        let dir = std::env::temp_dir().join("eazyqq_boot_test_missing");
        let _ = std::fs::create_dir_all(&dir);
        let err = resolve(&dir).unwrap_err();
        assert!(err.contains("NapCatWinBootMain.exe"), "got: {err}");
    }

    #[test]
    fn empty_qq_path_file_is_rejected() {
        let dir = std::env::temp_dir().join("eazyqq_boot_test_empty");
        let cfg = dir.join("config");
        let _ = std::fs::create_dir_all(&cfg);
        std::fs::write(cfg.join("qq_path.txt"), "   \n").unwrap();

        let err = configured_qq_path(&dir).unwrap_err();
        assert!(err.contains("为空"), "got: {err}");
    }

    #[test]
    fn qq_path_pointing_nowhere_is_rejected() {
        let dir = std::env::temp_dir().join("eazyqq_boot_test_nowhere");
        let cfg = dir.join("config");
        let _ = std::fs::create_dir_all(&cfg);
        std::fs::write(cfg.join("qq_path.txt"), r"Z:\definitely\not\here\QQ.exe").unwrap();

        let err = configured_qq_path(&dir).unwrap_err();
        assert!(err.contains("不存在"), "got: {err}");
    }

    #[test]
    fn restart_is_rate_limited() {
        // A backend that cannot start fails every time; without throttling this would
        // spawn processes forever.
        note_attempt();
        let blocked = may_attempt();
        assert!(blocked.is_err(), "a second immediate attempt must be refused");
        assert!(blocked.unwrap_err().contains("需等待"));
    }

    #[test]
    fn repeated_failures_stop_the_loop() {
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            note_result(false);
        }
        let blocked = may_attempt();
        assert!(blocked.is_err());
        assert!(blocked.unwrap_err().contains("停止自动重启"));
        assert_eq!(consecutive_failures(), MAX_CONSECUTIVE_FAILURES);

        // A healthy observation clears the counter so recovery is possible.
        note_healthy();
        assert_eq!(consecutive_failures(), 0);
    }
}
