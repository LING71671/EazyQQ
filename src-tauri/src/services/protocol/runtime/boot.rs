use std::sync::atomic::{AtomicBool, Ordering};
static AUTO_START: AtomicBool = AtomicBool::new(true);
pub fn set_auto_start(enabled: bool) {
    AUTO_START.store(enabled, Ordering::SeqCst);
}
pub fn auto_start_allowed() -> bool {
    AUTO_START.load(Ordering::SeqCst)
}
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::patch::{configured_qq_path, sync_qqnt_patch};
use serde::Serialize;

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
    pub instance_pids: std::collections::HashMap<String, u32>,
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

pub fn locate_napcat_dir() -> PathBuf {
    let root_dir = crate::services::logging::workspace_root();
    let mut candidates = vec![
        root_dir.join("napcat"),
        root_dir.join("resources").join("napcat"),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources").join("napcat"));
            candidates.push(exe_dir.join("napcat"));
        }
    }

    if let Some(parent) = root_dir.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("napcat") {
                        candidates.push(p);
                    }
                }
            }
        }
    }

    candidates
        .into_iter()
        .find(|p| p.join("NapCatWinBootMain.exe").exists() || p.join("napcat.mjs").exists())
        .unwrap_or_else(|| root_dir.join("napcat"))
}

pub fn resolve(napcat_dir: &Path) -> Result<BootFiles, String> {
    let launcher = napcat_dir.join("NapCatWinBootMain.exe");
    let hook_dll = napcat_dir.join("NapCatWinBootHook.dll");
    let main_mjs = napcat_dir.join("napcat.mjs");
    let load_js = napcat_dir.join("loadNapCat.cjs");
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
    set_auto_start(false);
    let source = locate_napcat_dir();
    let (dir, _, _, _) = crate::services::protocol::layout::selected(&source);
    stop_scoped(&dir)
}

pub fn stop_instance(uin: &str) -> BootOutcome {
    let reg = crate::services::identity::instances::load_registry();
    match reg.instances.iter().find(|i| i.uin == uin) {
        Some(inst) => {
            let outcome = stop_scoped(&crate::services::protocol::layout::runtime_dir(inst));
            if outcome.ok {
                let _ = crate::services::instances::configure_instance(uin, false);
            }
            outcome
        }
        None => BootOutcome {
            attempted: false,
            ok: false,
            detail: format!("Unknown account {}", uin),
        },
    }
}

pub fn stop_all_instances() -> Vec<BootOutcome> {
    let reg = crate::services::identity::instances::load_registry();
    let mut outcomes = Vec::new();
    for inst in &reg.instances {
        outcomes.push(stop_instance(&inst.uin));
    }
    let default_stop = stop();
    outcomes.push(default_stop);
    outcomes
}

pub fn restart(napcat_dir: &Path) -> BootOutcome {
    let account = crate::services::accounts::active();
    restart_with_uin(napcat_dir, account.as_deref())
}

pub fn restart_with_uin(napcat_dir: &Path, uin: Option<&str>) -> BootOutcome {
    if let Ok(mut st) = state().lock() {
        st.consecutive_failures = 0;
        st.last_attempt = None;
        st.awaiting_readiness = false;
    }
    let stopped = super::ownership::stop(napcat_dir);
    if !stopped.ok {
        return stopped;
    }
    for _ in 0..30 {
        if !super::ownership::is_running(napcat_dir) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if super::ownership::is_running(napcat_dir) {
        return BootOutcome {
            attempted: true,
            ok: false,
            detail: "The owned process tree has not finished stopping".into(),
        };
    }
    if !stopped.attempted
        && std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], webui_port(napcat_dir))),
            Duration::from_millis(300),
        )
        .is_ok()
    {
        return BootOutcome {
            attempted: false,
            ok: false,
            detail: "An unowned protocol session is running; restart was refused".into(),
        };
    }
    start_with_args(napcat_dir, None, uin)
}

pub fn start_instance(napcat_dir: &Path, uin: &str) -> BootOutcome {
    let _maintenance = match crate::services::infra::persistence::FileLock::shared(
        &crate::services::accounts::data_root().join("protocol-maintenance.lock"),
    ) {
        Ok(guard) => guard,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    let inst = match super::layout::resolve_instance(uin) {
        Ok(i) => i,
        Err(e) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail: e,
            }
        }
    };
    let dir = crate::services::protocol::layout::runtime_dir(&inst);
    let _guard = match crate::services::infra::persistence::FileLock::acquire(
        &dir.join("eazyqq-lifecycle.lock"),
    ) {
        Ok(guard) => guard,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    if super::ownership::is_running(&crate::services::protocol::layout::runtime_dir(&inst)) {
        return BootOutcome {
            attempted: false,
            ok: true,
            detail: "Account protocol process is already running".into(),
        };
    }
    let dir = match crate::services::protocol::layout::prepare(napcat_dir, &inst) {
        Ok(dir) => dir,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    start_unlocked(&dir, Some(&inst), Some(uin))
}

pub fn start(napcat_dir: &Path) -> BootOutcome {
    start_with_args(napcat_dir, None, None)
}

fn webui_port(dir: &Path) -> u16 {
    std::fs::read(dir.join("config/webui.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["port"].as_u64())
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or(6099)
}

pub fn start_with_args(
    napcat_dir: &Path,
    inst: Option<&crate::services::instances::AccountInstance>,
    target_uin: Option<&str>,
) -> BootOutcome {
    let _maintenance = match crate::services::infra::persistence::FileLock::shared(
        &crate::services::accounts::data_root().join("protocol-maintenance.lock"),
    ) {
        Ok(guard) => guard,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    let _guard = match crate::services::infra::persistence::FileLock::acquire(
        &napcat_dir.join("eazyqq-lifecycle.lock"),
    ) {
        Ok(guard) => guard,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    start_unlocked(napcat_dir, inst, target_uin)
}

fn start_unlocked(
    napcat_dir: &Path,
    inst: Option<&crate::services::identity::instances::AccountInstance>,
    target_uin: Option<&str>,
) -> BootOutcome {
    if super::ownership::is_running(napcat_dir) {
        return BootOutcome {
            attempted: false,
            ok: true,
            detail: "Protocol process is already running".into(),
        };
    }
    if std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], webui_port(napcat_dir))),
        Duration::from_millis(250),
    )
    .is_ok()
    {
        return BootOutcome {
            attempted: false,
            ok: false,
            detail: "The configured control port is occupied by an unowned service; its process was preserved".into(),
        };
    }
    if inst.is_none() {
        if let Err(reason) = may_attempt() {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail: reason,
            };
        }
    }
    if let Err(detail) = ensure_control_config(napcat_dir) {
        return BootOutcome {
            attempted: false,
            ok: false,
            detail,
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

    if let Some(uin) = target_uin {
        if let Err(detail) = super::session::validate_uin(uin) {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            };
        }
    }
    let main_url = match reqwest::Url::from_file_path(&files.main_mjs) {
        Ok(url) => url,
        Err(_) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail: "Invalid protocol module file URL".into(),
            }
        }
    };
    let loader = format!(
        "(async () => {{await import({})}})()",
        serde_json::to_string(main_url.as_str()).unwrap()
    );
    if let Err(e) = std::fs::write(&files.load_js, loader) {
        note_result(false);
        return BootOutcome {
            attempted: true,
            ok: false,
            detail: format!("写入 {} 失败: {}", files.load_js.display(), e),
        };
    }

    if let Err(detail) = sync_qqnt_patch(&files.qq_path, &files.patch_pkg) {
        note_result(false);
        return BootOutcome { attempted: false, ok: false, detail };
    }
    note_attempt();

    let mut cmd = Command::new(&files.launcher);
    cmd.arg(&files.qq_path).arg(&files.hook_dll);

    let explicit_uin = target_uin.or_else(|| inst.map(|i| i.uin.as_str()));
    if let Some(u) = explicit_uin {
        cmd.arg("-q").arg(u);
    }

    cmd.current_dir(napcat_dir)
        .env("NAPCAT_WORKDIR", napcat_dir)
        .env("NAPCAT_PATCH_PACKAGE", &files.patch_pkg)
        .env("NAPCAT_LOAD_PATH", &files.load_js)
        .env("NAPCAT_INJECT_PATH", &files.hook_dll)
        .env("NAPCAT_LAUNCHER_PATH", &files.launcher)
        .env("NAPCAT_MAIN_PATH", &files.main_mjs)
        .env(
            "QQ_PATH_CONFIG",
            napcat_dir.join("config").join("qq_path.txt"),
        );

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

    let spawn = super::ownership::spawn_owned(&mut cmd, napcat_dir, output_log.as_deref());

    match spawn {
        Ok(pid) => {
            if let Ok(mut st) = state().lock() {
                if inst.is_none() {
                    st.child_pid = Some(pid);
                }
                if let Some(i) = inst {
                    st.instance_pids.insert(i.uin.clone(), pid);
                }
            }
            if let Some(i) = inst {
                let _ =
                    crate::services::identity::instances::update_instance_pid(&i.uin, Some(pid));
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
                detail: match (inst, &output_log) {
                    (Some(i), Some(p)) => format!(
                        "已拉起账号 {} (PID {}, HTTP 端口:{}, WS 端口:{})，日志写入 {}",
                        i.uin,
                        pid,
                        i.http_port,
                        i.ws_port,
                        p.display()
                    ),
                    (Some(i), None) => format!(
                        "已拉起账号 {} (PID {}, HTTP 端口:{}, WS 端口:{})",
                        i.uin, pid, i.http_port, i.ws_port
                    ),
                    (None, Some(p)) => format!(
                        "已拉起 NapCat（QQ: {}），输出写入 {}",
                        files.qq_path.display(),
                        p.display()
                    ),
                    (None, None) => format!("已拉起 NapCat（QQ: {}）", files.qq_path.display()),
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

fn ensure_control_config(dir: &Path) -> Result<(), String> {
    let path = dir.join("config/webui.json");
    let mut config = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|e| format!("WebUI configuration is invalid: {}", e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            serde_json::json!({"host":"127.0.0.1","port":6099,"autoLoginAccount":""})
        }
        Err(e) => return Err(e.to_string()),
    };
    if config["token"]
        .as_str()
        .is_none_or(|token| token.is_empty())
    {
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        config["token"] = random
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>()
            .into();
        crate::services::infra::persistence::write_json(&path, &config)?;
    }
    Ok(())
}

fn stop_scoped(dir: &Path) -> BootOutcome {
    let _guard = match crate::services::infra::persistence::FileLock::acquire(
        &dir.join("eazyqq-lifecycle.lock"),
    ) {
        Ok(guard) => guard,
        Err(detail) => {
            return BootOutcome {
                attempted: false,
                ok: false,
                detail,
            }
        }
    };
    let outcome = super::ownership::stop(dir);
    if !outcome.attempted
        && std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], webui_port(dir))),
            Duration::from_millis(250),
        )
        .is_ok()
    {
        return BootOutcome {
            attempted: false,
            ok: false,
            detail:
                "An externally managed protocol session is running. Its QQ process was preserved"
                    .into(),
        };
    }
    outcome
}
