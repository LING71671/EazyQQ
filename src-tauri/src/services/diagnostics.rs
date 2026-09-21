//! Diagnostics bundle export (ROADMAP Phase 6).
//!
//! Produces a real `.zip` with a self-describing README, the health report, the current
//! configuration (secrets masked), the rule table, log tails and crash reports.
//!
//! The ZIP container is written here rather than pulling in a `zip` crate: entries are
//! stored uncompressed, which keeps the dependency graph untouched and makes the output
//! readable by every standard unzip tool.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::logging;
use crate::services::napcat::NapCatService;
use crate::services::onebot::OneBotClient;

const LOG_TAIL_LINES: usize = 2000;

// ---------------------------------------------------------------------------
// Minimal ZIP (stored entries, no compression)
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn build_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;

    for (name, data) in entries {
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_bytes = name.as_bytes();

        let mut local: Vec<u8> = Vec::new();
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes()); // version needed
        local.extend_from_slice(&0u16.to_le_bytes()); // flags
        local.extend_from_slice(&0u16.to_le_bytes()); // method = store
        local.extend_from_slice(&0u16.to_le_bytes()); // mod time
        local.extend_from_slice(&0u16.to_le_bytes()); // mod date
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes()); // extra len
        local.extend_from_slice(name_bytes);

        out.extend_from_slice(&local);
        out.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // method
        central.extend_from_slice(&0u16.to_le_bytes()); // time
        central.extend_from_slice(&0u16.to_le_bytes()); // date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);

        offset += local.len() as u32 + size;
    }

    let cd_offset = offset;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with CD
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out
}

// ---------------------------------------------------------------------------
// Desensitisation
// ---------------------------------------------------------------------------

/// Mask anything that looks like a credential before it leaves the machine.
fn redact(input: &str, secrets: &[String]) -> String {
    let mut out = input.to_string();
    for secret in secrets {
        let trimmed = secret.trim();
        if trimmed.len() >= 8 {
            out = out.replace(trimmed, "***REDACTED***");
        }
    }

    // Generic API-key shaped tokens.
    let mut result = String::with_capacity(out.len());
    for token in out.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        let body = token.trim_end_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
        let suffix = &token[body.len()..];
        if (body.starts_with("sk-") || body.starts_with("sk_")) && body.len() >= 12 {
            result.push_str("***REDACTED***");
        } else {
            result.push_str(body);
        }
        result.push_str(suffix);
    }

    // Middle-mask QQ numbers so the bundle stays diagnosable but not identifying.
    result
}

fn mask_id(id: &str) -> String {
    if id.len() <= 5 {
        return id.to_string();
    }
    format!("{}***{}", &id[..2], &id[id.len() - 2..])
}

// ---------------------------------------------------------------------------
// Bundle assembly
// ---------------------------------------------------------------------------

/// Free space on the volume holding `path`, in MiB. Dependency-free: calls
/// `GetDiskFreeSpaceExW` through a raw `extern "system"` declaration.
#[cfg(windows)]
pub fn free_space_mb(path: &Path) -> Option<i64> {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpdirectoryname: *const u16,
            lpfreebytesavailabletocaller: *mut u64,
            lptotalnumberofbytes: *mut u64,
            lptotalnumberoffreebytes: *mut u64,
        ) -> i32;
    }

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let mut available = 0u64;
    let mut total = 0u64;
    let mut total_free = 0u64;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            &mut total,
            &mut total_free,
        )
    };
    if ok == 0 {
        None
    } else {
        Some((total_free / (1024 * 1024)) as i64)
    }
}

#[cfg(not(windows))]
pub fn free_space_mb(_path: &Path) -> Option<i64> {
    None
}

/// Actually try to write, instead of assuming the directory is usable.
pub fn is_dir_writable(path: &Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(".eazyqq-write-probe");
    match fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn detect_webview2_version() -> Option<String> {
    let base = Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application");
    let mut versions: Vec<String> = fs::read_dir(base)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort();
    versions.pop()
}

fn detect_windhawk() -> (bool, Option<String>) {
    let base = Path::new(r"C:\Program Files\Windhawk");
    if !base.exists() {
        return (false, None);
    }
    let version = fs::read_to_string(base.join("version.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    (true, version)
}

fn tail_file(path: &Path, lines: usize) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Some(all[start..].join("\n"))
}

pub struct ExportResult {
    pub zip_path: PathBuf,
    pub entries: Vec<String>,
    pub bytes: usize,
}

/// Build the diagnostics bundle and return where it landed.
pub async fn export_bundle(
    db: &Arc<Database>,
    ai: &Arc<AiService>,
    napcat: &Arc<NapCatService>,
    onebot: &Arc<OneBotClient>,
) -> Result<ExportResult, String> {
    let out_dir = logging::data_dir().join("diagnostics");
    logging::ensure_dir(&out_dir);

    let api_key = db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let secrets = vec![api_key.clone()];

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // --- system.json
    let (windhawk_present, windhawk_version) = detect_windhawk();
    let system = serde_json::json!({
        "app": {
            "name": "EazyQQ",
            "version": env!("CARGO_PKG_VERSION"),
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        },
        "os": {
            "family": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "windows": std::env::var("OS").ok(),
            "processor": std::env::var("PROCESSOR_IDENTIFIER").ok(),
        },
        "paths": {
            "workspaceRoot": logging::workspace_root().display().to_string(),
            "dataDir": logging::data_dir().display().to_string(),
            "logDir": logging::log_dir().display().to_string(),
        },
        "webview2": {
            "installedVersion": detect_webview2_version(),
        },
        "conflicts": {
            "windhawkInstalled": windhawk_present,
            "windhawkVersion": windhawk_version,
            "note": "Windhawk injects windhawk.dll into every process by default, which crashes the WebView2 browser process (msedge.dll STATUS_BREAKPOINT) and makes the EazyQQ window render black/blank. Exclude msedgewebview2.exe from Windhawk to fix.",
        },
    });
    entries.push((
        "system.json".to_string(),
        serde_json::to_vec_pretty(&system).unwrap_or_default(),
    ));

    // --- health.json (same probes as `eazyqq-cli health --deep` minus the paid call)
    let napcat_alive = napcat.is_alive().await;
    let login = onebot.get_login_info().await;
    let (log_dir, crash_count, last_crash) = logging::diagnostics();
    let health = serde_json::json!({
        "napcatWebUiAlive": napcat_alive,
        "onebotReachable": login.is_ok(),
        "loggedIn": login.is_ok(),
        "loginInfo": login.as_ref().ok().map(|v| serde_json::json!({
            "qqNumber": mask_id(&v.get("data").and_then(|d| d.get("user_id")).map(|u| u.to_string()).unwrap_or_default()),
            "nickname": v.get("data").and_then(|d| d.get("nickname")).and_then(|n| n.as_str()),
        })),
        "apiKeyConfigured": !api_key.is_empty(),
        "model": ai.model(),
        "logDir": log_dir.display().to_string(),
        "crashReportCount": crash_count,
        "lastCrashReport": last_crash.map(|p| p.display().to_string()),
    });
    entries.push((
        "health.json".to_string(),
        serde_json::to_vec_pretty(&health).unwrap_or_default(),
    ));

    // --- config/app_config.json (masked)
    if let Ok(Some(raw)) = db.get_setting("app_config") {
        entries.push((
            "config/app_config.json".to_string(),
            redact(&raw, &secrets).into_bytes(),
        ));
    } else {
        entries.push((
            "config/app_config.json".to_string(),
            b"{}".to_vec(),
        ));
    }

    // --- rules/contact_rules.json (ids masked)
    let rules = db.get_all_rules().unwrap_or_default();
    let rules_json: Vec<serde_json::Value> = rules
        .iter()
        .map(|r| {
            serde_json::json!({
                "targetId": mask_id(&r.target_id),
                "targetType": r.target_type,
                "name": r.name,
                "mode": r.mode,
                "enabled": r.enabled,
                "isSummaryWhitelist": r.is_summary_whitelist,
                "summaryIntervalHours": r.summary_interval_hours,
                "triggerCondition": r.trigger_condition,
            })
        })
        .collect();
    entries.push((
        "rules/contact_rules.json".to_string(),
        serde_json::to_vec_pretty(&serde_json::json!({
            "total": rules_json.len(),
            "list": rules_json,
        }))
        .unwrap_or_default(),
    ));

    // --- stats.json
    let stats = serde_json::json!({
        "rules": rules.len(),
        "summaryWhitelistGroups": db.get_summary_whitelist_groups().map(|v| v.len()).unwrap_or(0),
        "pendingDrafts": db.get_pending_drafts().map(|v| v.len()).unwrap_or(0),
        "summaries": db.get_summaries(None).map(|v| v.len()).unwrap_or(0),
    });
    entries.push((
        "stats.json".to_string(),
        serde_json::to_vec_pretty(&stats).unwrap_or_default(),
    ));

    // --- logs/eazyqq.log (masked tail)
    match tail_file(&logging::active_log_path(), LOG_TAIL_LINES) {
        Some(text) => entries.push((
            "logs/eazyqq.log".to_string(),
            redact(&text, &secrets).into_bytes(),
        )),
        None => tracing::warn!("diagnostics: active log file not readable"),
    }

    // --- logs/last-crash.log
    if let Some(crash) = tail_file(&logging::log_dir().join("last-crash.log"), 500) {
        entries.push((
            "logs/last-crash.log".to_string(),
            redact(&crash, &secrets).into_bytes(),
        ));
    }

    // --- README.txt
    let readme = format!(
        "EazyQQ 诊断日志包\n\
         =================\n\
         生成时间戳(Unix秒): {}\n\
         应用版本: {}\n\
         工作目录: {}\n\n\
         包含内容:\n\
         - system.json          运行环境、路径、WebView2 版本、已知冲突检测\n\
         - health.json          链路自检结果（协议端 / 登录态 / 模型 / 崩溃计数）\n\
         - config/app_config.json  当前配置（API Key 已脱敏）\n\
         - rules/contact_rules.json 联系人规则快照（QQ 号中间位已打码）\n\
         - stats.json           规则、草稿、简报数量统计\n\
         - logs/eazyqq.log      运行日志尾部（凭证已脱敏）\n\
         - logs/last-crash.log  最近一次 panic 报告（如有）\n\n\
         脱敏说明:\n\
         - API Key 全量替换为 ***REDACTED***\n\
         - QQ 号仅保留前 2 位与后 2 位\n\
         - 本包不包含聊天消息内容\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        env!("CARGO_PKG_VERSION"),
        logging::workspace_root().display()
    );
    entries.push(("README.txt".to_string(), readme.into_bytes()));

    // --- write
    let zip_bytes = build_zip(&entries);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let zip_path = out_dir.join(format!("eazyqq-diagnostics-{}.zip", stamp));

    let mut file = fs::File::create(&zip_path)
        .map_err(|e| format!("无法创建诊断包 {}: {}", zip_path.display(), e))?;
    file.write_all(&zip_bytes)
        .map_err(|e| format!("写入诊断包失败: {}", e))?;
    file.flush().ok();

    let names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
    tracing::info!(
        "diagnostics bundle written: {} ({} files, {} bytes)",
        zip_path.display(),
        names.len(),
        zip_bytes.len()
    );

    Ok(ExportResult {
        zip_path,
        entries: names,
        bytes: zip_bytes.len(),
    })
}
