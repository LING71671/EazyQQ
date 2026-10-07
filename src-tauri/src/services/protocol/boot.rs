use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use super::patch::{configured_qq_path, sync_qqnt_patch};

const MIN_RESTART_INTERVAL: Duration = Duration::from_secs(90);
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct BootOutcome {
    pub attempted: bool,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Default)]
pub struct BootState {
    pub last_attempt: Option<Instant>,
    pub consecutive_failures: u32,
    pub awaiting_readiness: bool,
    pub child_pid: Option<u32>,
}

pub fn state() -> &'static Mutex<BootState> {
    static S: std::sync::OnceLock<Mutex<BootState>> = std::sync::OnceLock::new();
    S.get_or_init(|| Mutex::new(BootState::default()))
}

#[derive(Debug)]
pub struct BootFiles {
    pub launcher: PathBuf,
    pub hook_dll: PathBuf,
    pub main_mjs: PathBuf,
    pub load_js: PathBuf,
    pub patch_pkg: PathBuf,
    pub qq_path: PathBuf,
}

pub fn resolve(napcat_dir: &Path) -> Result<BootFiles, String> {
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

pub fn may_attempt() -> Result<(), String> {
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

pub fn note_attempt() {
    if let Ok(mut st) = state().lock() {
        st.last_attempt = Some(Instant::now());
    }
}

pub fn note_result(ok: bool) {
    if let Ok(mut st) = state().lock() {
        if ok {
            st.consecutive_failures = 0;
        } else {
            st.consecutive_failures += 1;
        }
    }
}

fn next_output_log_path() -> Option<PathBuf> {
    let dir = crate::services::logging::data_dir().join("napcat_logs");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(dir.join(format!("boot-{}.log", stamp)))
}

pub fn running_qq_processes() -> Vec<String> {
    let mut cmd = Command::new("tasklist");
    cmd.args(["/FI", "IMAGENAME eq QQ.exe", "/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match cmd.output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.contains("QQ.exe"))
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn note_launch_pending() {
    if let Ok(mut st) = state().lock() {
        st.awaiting_readiness = true;
    }
}

pub fn note_unready() {
    if let Ok(mut st) = state().lock() {
        if st.awaiting_readiness {
            st.awaiting_readiness = false;
            st.consecutive_failures += 1;
        }
    }
}

pub fn note_healthy() {
    if let Ok(mut st) = state().lock() {
        st.consecutive_failures = 0;
        st.awaiting_readiness = false;
    }
}

pub fn consecutive_failures() -> u32 {
    state().lock().map(|s| s.consecutive_failures).unwrap_or(0)
}

pub fn stop() -> BootOutcome {
    let mut killed_any = false;
    let mut detail_msgs = Vec::new();

    if let Ok(mut st) = state().lock() {
        if let Some(pid) = st.child_pid.take() {
            #[cfg(target_os = "windows")]
            {
                let output = std::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output();
                if let Ok(o) = output {
                    if o.status.success() {
                        killed_any = true;
                        detail_msgs.push(format!("已清理 NapCat 进程树 (PID {})", pid));
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "NapCatWinBootMain.exe"])
            .output();
        if let Ok(o) = output {
            if o.status.success() {
                killed_any = true;
                detail_msgs.push("已清理残留 NapCat 引导进程".to_string());
            }
        }
    }

    BootOutcome {
        attempted: true,
        ok: true,
        detail: if killed_any {
            detail_msgs.join("; ")
        } else {
            "未检测到运行中的 NapCat 专属进程".to_string()
        },
    }
}

pub fn restart(napcat_dir: &Path) -> BootOutcome {
    if let Ok(mut st) = state().lock() {
        st.consecutive_failures = 0;
        st.last_attempt = None;
        st.awaiting_readiness = false;
    }
    stop();
    start(napcat_dir)
}

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

    sync_qqnt_patch(&files.qq_path, &files.patch_pkg);
    note_attempt();

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

    let mut cmd = Command::new(&files.launcher);
    cmd.arg(&files.qq_path)
        .arg(&files.hook_dll)
        .current_dir(napcat_dir)
        .env("NAPCAT_PATCH_PACKAGE", &files.patch_pkg)
        .env("NAPCAT_LOAD_PATH", &files.load_js)
        .env("NAPCAT_INJECT_PATH", &files.hook_dll)
        .env("NAPCAT_LAUNCHER_PATH", &files.launcher)
        .env("NAPCAT_MAIN_PATH", &files.main_mjs)
        .env("QQ_PATH_CONFIG", napcat_dir.join("config").join("qq_path.txt"));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output_log = next_output_log_path();
    if let Some(path) = &output_log {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(file) = std::fs::File::create(path) {
            match file.try_clone() {
                Ok(dup) => {
                    cmd.stdout(std::process::Stdio::from(file));
                    cmd.stderr(std::process::Stdio::from(dup));
                }
                Err(_) => {
                    cmd.stdout(std::process::Stdio::from(file));
                }
            }
        }
    }

    let spawn = cmd.spawn();

    match spawn {
        Ok(child) => {
            let pid = child.id();
            if let Ok(mut st) = state().lock() {
                st.child_pid = Some(pid);
            }
            tracing::info!(
                "napcat boot: launched {} (pid {}) with QQ {}",
                files.launcher.display(),
                pid,
                files.qq_path.display()
            );
            note_launch_pending();
            BootOutcome {
                attempted: true,
                ok: true,
                detail: match &output_log {
                    Some(p) => format!(
                        "已拉起 NapCat（QQ: {}），输出写入 {}",
                        files.qq_path.display(),
                        p.display()
                    ),
                    None => format!("已拉起 NapCat（QQ: {}）", files.qq_path.display()),
                },
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
