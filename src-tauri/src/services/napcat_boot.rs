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
    /// A launch has been made and readiness has not been confirmed yet.
    awaiting_readiness: bool,
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

/// One step of the path from "the app wants to talk to QQ" to "NapCat answers on its WebUI".
#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

impl Step {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, ok: true, detail: detail.into() }
    }

    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, ok: false, detail: detail.into() }
    }
}

/// Walk the whole protocol-side start-up path, reporting every step.
///
/// One opaque "NapCat WebUI is DOWN" was hiding a chain of unrelated failures that each need
/// a different fix: a launcher invoked with the wrong arguments, a QQ path that did not
/// resolve, a QQ already running so the instance NapCat started exited immediately, and a
/// NapCat that loaded but never executed its own JavaScript. The monitor can now name the
/// step that is actually blocking instead of reporting the symptom.
pub fn diagnose(napcat_dir: &Path, uin: Option<&str>) -> Vec<Step> {
    let mut steps = Vec::new();

    // 1. The directory itself.
    steps.push(if napcat_dir.is_dir() {
        Step::ok("NapCat 目录", napcat_dir.display().to_string())
    } else {
        Step::fail("NapCat 目录", format!("不存在: {}", napcat_dir.display()))
    });

    // 2. The three files the launcher needs.
    let required = [
        ("NapCatWinBootMain.exe", "启动引导器"),
        ("NapCatWinBootHook.dll", "注入钩子"),
        ("napcat.mjs", "NapCat 主体"),
    ];
    let missing: Vec<String> = required
        .iter()
        .filter(|(f, _)| !napcat_dir.join(f).is_file())
        .map(|(f, what)| format!("{}（{}）", f, what))
        .collect();
    steps.push(if missing.is_empty() {
        Step::ok("必需文件", format!("{} 项齐全", required.len()))
    } else {
        Step::fail("必需文件", format!("缺少 {}", missing.join("、")))
    });

    // 3. Which QQ the launcher will start.
    let qq_path = configured_qq_path(napcat_dir);
    match &qq_path {
        Ok(p) => steps.push(Step::ok("QQ 路径配置", p.display().to_string())),
        Err(e) => steps.push(Step::fail("QQ 路径配置", e.clone())),
    }

    // 4. Which QQ build that installation actually is.
    if let Ok(p) = &qq_path {
        let mut versions: Vec<String> = p
            .parent()
            .map(|d| d.join("versions"))
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        versions.sort();
        steps.push(if versions.is_empty() {
            Step::fail("QQ 版本", "versions/ 下没有版本目录，QQ 安装可能不完整")
        } else {
            Step::ok("QQ 版本", versions.join("、"))
        });
    }

    // 5. Is QQ already running?
    //
    // Stated as the most likely cause rather than a proven one. What is measured: four new QQ
    // processes appear when NapCat starts, port 6099 is bound for about three seconds, then
    // all four exit and the WebUI goes with them - and the same happens with the sandbox out
    // of the picture, so it is not an artefact of how this was tested. NapCat boots its own QQ
    // (official docs) and ships `KillQQ.bat`, which is why a running QQ is the leading
    // explanation. It is not the only possible one, so it is not presented as settled.
    let running = running_qq_processes();
    steps.push(if running.is_empty() {
        Step::ok("QQ 进程", "未在运行，NapCat 可以自行引导 QQ 启动")
    } else {
        Step::fail(
            "QQ 进程",
            format!(
                "已有 {} 个 QQ 实例在运行。NapCat 会自行引导一个 QQ 实例启动，两者可能互相\
                 冲突——实测中 NapCat 拉起的 4 个 QQ 进程约 3 秒后全部退出，WebUI 随之消失。\
                 这是目前最可能的原因，但尚未定论。若 NapCat 反复启动失败，请先完全退出 QQ \
                 再试；退出后应用会自动把它拉起来。",
                running.len()
            ),
        )
    });

    // 6. Account-scoped configuration.
    if let Some(uin) = uin.filter(|u| !u.is_empty()) {
        let napcat_cfg = napcat_dir.join("config").join(format!("napcat_{}.json", uin));
        let onebot_cfg = napcat_dir.join("config").join(format!("onebot11_{}.json", uin));
        let absent: Vec<&str> = [
            (&napcat_cfg, "napcat_<uin>.json"),
            (&onebot_cfg, "onebot11_<uin>.json"),
        ]
        .iter()
        .filter(|(p, _)| !p.is_file())
        .map(|(_, what)| *what)
        .collect();
        steps.push(if absent.is_empty() {
            Step::ok("账号配置", format!("{} 的配置已就绪", uin))
        } else {
            Step::fail("账号配置", format!("缺少 {}", absent.join("、")))
        });
    }

    // 7. WebUI endpoint, which is what everything else dials.
    let webui_cfg = napcat_dir.join("config").join("webui.json");
    match std::fs::read_to_string(&webui_cfg)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    {
        Some(v) => {
            let host = v.get("host").and_then(|h| h.as_str()).unwrap_or("?");
            let port = v.get("port").and_then(|p| p.as_u64()).unwrap_or(0);
            let has_token = v
                .get("token")
                .and_then(|t| t.as_str())
                .map(|t| !t.is_empty())
                .unwrap_or(false);
            steps.push(Step::ok(
                "WebUI 配置",
                format!("{}:{}{}", host, port, if has_token { "（含 token）" } else { "（无 token）" }),
            ));
        }
        None => steps.push(Step::fail(
            "WebUI 配置",
            format!("无法读取 {}", webui_cfg.display()),
        )),
    }

    // 8. Did NapCat's own logging ever run?
    //
    // Deliberately reports only what can be observed. An earlier version of this text
    // concluded "NapCat's JavaScript never executed" from the empty directory, and that was
    // wrong: NapCat creates `logs/` and `cache/` itself, and it writes `cache/qrcode.png`,
    // so its JavaScript demonstrably does run. What the missing log actually tells us is
    // narrower - it exits before writing its first log line.
    let log_dir = napcat_dir.join("logs");
    let newest = std::fs::read_dir(&log_dir)
        .ok()
        .and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter_map(|m| m.modified().ok())
                .max()
        });
    steps.push(match newest {
        Some(t) => {
            let age = t.elapsed().map(|d| d.as_secs()).unwrap_or(u64::MAX);
            Step::ok("NapCat 自身日志", format!("最近写入于 {} 秒前", age))
        }
        None => Step::fail(
            "NapCat 自身日志",
            format!(
                "{} 下没有任何日志，而配置里 fileLog 是开启的。注意这只说明它没来得及写下\
                 第一条日志——NapCat 会自行创建该目录并生成 cache/qrcode.png，所以它的\
                 JavaScript 确实运行过，只是很快就退出了。",
                log_dir.display()
            ),
        ),
    });

    steps
}

/// The first step that failed, which is the one worth acting on.
pub fn first_blocker(napcat_dir: &Path, uin: Option<&str>) -> Option<Step> {
    diagnose(napcat_dir, uin).into_iter().find(|s| !s.ok)
}

/// Where to write this launch's console output.
///
/// Under the account's own `napcat_logs/`, alongside the other protocol-side logs, so it is
/// swept with the rest of the account's data and never leaks across accounts.
fn next_output_log_path() -> Option<PathBuf> {
    let dir = crate::services::logging::data_dir().join("napcat_logs");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(dir.join(format!("boot-{}.log", stamp)))
}

/// Running `QQ.exe` processes, as raw `tasklist` lines.
///
/// Shelling out keeps this dependency-free; the alternative is a process-enumeration crate
/// for one diagnostic.
fn running_qq_processes() -> Vec<String> {
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

/// Record that a launch was made and its readiness is still unknown.
fn note_launch_pending() {
    if let Ok(mut st) = state().lock() {
        st.awaiting_readiness = true;
    }
}

/// Record that a launched NapCat never became reachable.
///
/// The missing half of `start()`: a spawn proves a process exists, not that NapCat works.
/// Called by the chain monitor when the probe still fails after a launch, so that a backend
/// which can never start eventually stops being retried instead of spawning QQ forever.
pub fn note_unready() {
    if let Ok(mut st) = state().lock() {
        if st.awaiting_readiness {
            st.awaiting_readiness = false;
            st.consecutive_failures += 1;
        }
    }
}

/// Reset the failure counter (called when the backend is observed healthy again).
pub fn note_healthy() {
    if let Ok(mut st) = state().lock() {
        st.consecutive_failures = 0;
        st.awaiting_readiness = false;
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

    // Note whether QQ is already running, but do not refuse to start because of it.
    //
    // NapCat boots its own QQ ("NapCat 可以完全自身引导 QQ 程序的启动" - official docs), so a
    // running QQ client is a plausible reason for that instance to exit: measured, four new
    // QQ processes appear, port 6099 is bound for about three seconds, then all four exit and
    // the WebUI goes with them. NapCat also ships `KillQQ.bat` (`taskkill /f /im QQ.exe`).
    //
    // But "plausible" is not "proven", and refusing outright would break a setup that works.
    // So this only records the hint; the retry budget below is what stops the churn, and it
    // does so for any cause.
    let qq_running = running_qq_processes().len();
    if qq_running > 0 {
        tracing::warn!(
            "napcat boot: {} QQ process(es) already running; NapCat boots its own QQ, so if \
             this start fails, quitting QQ is the first thing to try",
            qq_running
        );
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

    // Capture the launcher's output.
    //
    // NapCat runs with `consoleLog: true`, so it prints what it is doing and, when it stops
    // early, why. All of that was going to the app's inherited stdout and being thrown away,
    // which left a failing boot undiagnosable from the app side - the reason a whole
    // investigation had to be done by hand from the outside.
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
            tracing::info!(
                "napcat boot: launched {} (pid {}) with QQ {}",
                files.launcher.display(),
                child.id(),
                files.qq_path.display()
            );
            // The bootstrapper hands off to QQ and exits, so a successful spawn only means
            // "started", not "ready" - the chain monitor decides that, and calls
            // `note_unready` when it never becomes ready. Nothing used to be recorded here,
            // which is why `consecutive_failures` never moved and the "stop after three
            // attempts" guard could never fire.
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
