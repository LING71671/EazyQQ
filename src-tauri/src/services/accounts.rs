//! Per-account data isolation.
//!
//! Everything the app stores about a person is private to the QQ account it belongs to:
//! message history, whitelist rules, AI drafts, group summaries, downloaded group files
//! and - worst of all if shared - the log file, which contains message text verbatim.
//!
//! Originally all of that lived directly under `EazyQQ_Data/`, so signing in with a
//! different QQ account exposed the previous account's data. NapCat itself already
//! partitions its config per account (`napcat_<uin>.json`), so the layout here follows the
//! same idea:
//!
//! ```text
//! EazyQQ_Data/
//!   bootstrap.json          app-level only: last account, WebView compat flag
//!   accounts/
//!     <uin>/
//!       eazyqq.db           rules, messages, drafts, summaries
//!       logs/               contains message content - must never be shared
//!       group_files/
//!       diagnostics/
//!     _unbound/             used before any account is known
//! ```
//!
//! `bootstrap.json` stays at the root because it has to be readable *before* an account is
//! known - the WebView compatibility flag is applied before the window is created, and the
//! account is only discovered once the protocol side answers.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::services::logging;

/// Directory used before a QQ account is known.
pub const UNBOUND: &str = "_unbound";

/// App-level state that must be readable without knowing the account.
///
/// `rename_all` is required: this file is hand-editable and read by other tools, so its
/// field names are camelCase. Without it serde looks for `last_account` and silently gets
/// `None` from a file that says `lastAccount` - which is exactly the bug this comment
/// exists to prevent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    /// The account whose data directory this process should use.
    #[serde(default)]
    pub last_account: Option<String>,
    /// Adds `--no-sandbox` to the WebView2 command line. App-level because it must be
    /// applied before the window exists, and the account is not known yet at that point.
    #[serde(default)]
    pub webview_compat_mode: bool,
}

pub fn data_root() -> PathBuf {
    logging::workspace_root().join("EazyQQ_Data")
}

pub fn bootstrap_path() -> PathBuf {
    data_root().join("bootstrap.json")
}

/// Read the bootstrap file. A missing or unreadable file is not an error: it simply means
/// this is a fresh install, which is a perfectly normal state.
pub fn read_bootstrap() -> Bootstrap {
    match std::fs::read_to_string(bootstrap_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(
                "accounts: {} is not valid JSON ({}); starting from defaults",
                bootstrap_path().display(),
                e
            );
            Bootstrap::default()
        }),
        Err(_) => Bootstrap::default(),
    }
}

pub fn write_bootstrap(b: &Bootstrap) -> Result<(), String> {
    let path = bootstrap_path();
    if let Some(parent) = path.parent() {
        logging::ensure_dir(parent);
    }
    let text = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("写入 {} 失败: {}", path.display(), e))
}

/// Update just the WebView compatibility flag, leaving the account untouched.
pub fn set_webview_compat(enabled: bool) -> Result<(), String> {
    let mut b = read_bootstrap();
    b.webview_compat_mode = enabled;
    write_bootstrap(&b)
}

// --- machine identity ------------------------------------------------------------------

/// Minimal binding so a real machine identifier can be read without pulling in a new
/// dependency. `RegGetValueW` is in advapi32.
#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            pdwtype: *mut u32,
            pvdata: *mut core::ffi::c_void,
            pcbdata: *mut u32,
        ) -> i32;
    }

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` - stable across reboots and
    /// hardware changes, and readable without elevation.
    pub fn machine_guid() -> Option<String> {
        let subkey = wide(r"SOFTWARE\Microsoft\Cryptography");
        let value = wide("MachineGuid");
        let mut buf = vec![0u16; 256];
        let mut size = (buf.len() * 2) as u32;

        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut size,
            )
        };
        if rc != 0 {
            return None;
        }

        let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..len]);
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn machine_guid() -> Option<String> {
        None
    }
}

/// A short, stable token identifying this machine.
///
/// Data is keyed by machine as well as account because the workspace can be copied or
/// synced: without this, restoring a folder onto a second machine would present the first
/// machine's private data as if it belonged to whoever signs in there.
///
/// Deliberately *not* a cryptographic hash - it only names a directory, and a plain
/// FNV-1a keeps this dependency-free.
pub fn machine_id() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let source = win::machine_guid()
                .or_else(|| std::env::var("COMPUTERNAME").ok())
                .or_else(|| std::env::var("HOSTNAME").ok())
                .unwrap_or_else(|| "unknown-machine".to_string());
            format!("{:016x}", fnv1a64(source.trim().as_bytes()))
        })
        .clone()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Where a given account's data lives on this machine. `None` maps to the unbound
/// directory.
pub fn account_dir(uin: Option<&str>) -> PathBuf {
    let leaf = match uin {
        Some(u) if !u.trim().is_empty() => sanitize_account(u),
        _ => UNBOUND.to_string(),
    };
    data_root()
        .join("accounts")
        .join(machine_id())
        .join(leaf)
}

/// Account ids come from the protocol side, so treat them as untrusted input: a value like
/// `../../` must not be able to escape the accounts directory.
fn sanitize_account(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(32)
        .collect();
    if cleaned.is_empty() {
        UNBOUND.to_string()
    } else {
        cleaned
    }
}

// --- process-wide active account -------------------------------------------------------

fn active_slot() -> &'static Mutex<Option<String>> {
    static S: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// The account this process is serving. Set once during startup, before any data path is
/// computed, so every later `data_dir()` call is consistent.
pub fn active() -> Option<String> {
    active_slot().lock().ok().and_then(|s| s.clone())
}

pub fn set_active(uin: Option<String>) {
    if let Ok(mut s) = active_slot().lock() {
        *s = uin;
    }
}

/// The data directory for the active account.
pub fn active_data_dir() -> PathBuf {
    account_dir(active().as_deref())
}

/// Create the directory layout for an account and return its root.
pub fn ensure_layout(uin: Option<&str>) -> Result<PathBuf, String> {
    let dir = account_dir(uin);
    for sub in ["", "logs", "group_files", "diagnostics"] {
        let path = if sub.is_empty() { dir.clone() } else { dir.join(sub) };
        logging::ensure_dir(&path);
    }
    Ok(dir)
}

/// Did this account switch since the process started?
///
/// A different account means every path computed at startup is now wrong, so the caller
/// must persist the new account and restart rather than silently reusing the old data
/// directory.
pub fn account_changed(observed: &str) -> bool {
    match active() {
        Some(current) => current != observed,
        None => false,
    }
}

/// Adopt an account: persist it and make it active for this process.
pub fn adopt(uin: &str) -> Result<(), String> {
    let mut b = read_bootstrap();
    b.last_account = Some(uin.to_string());
    write_bootstrap(&b)?;
    set_active(Some(uin.to_string()));
    Ok(())
}

/// Move pre-isolation data into the account directory that owns it.
///
/// The old layout kept one `eazyqq.db`, one log file and one group-files directory directly
/// under `EazyQQ_Data/`. Those all belong to whichever account was in use, so on first run
/// with the new layout they are moved into that account's directory rather than abandoned.
/// Without this, upgrading would look like "all my rules and drafts disappeared".
pub fn migrate_legacy_if_needed(uin: &str) -> Result<Vec<String>, String> {
    let root = data_root();
    let dest = account_dir(Some(uin));
    let mut moved = Vec::new();

    // Deliberately not gated on "does the account already have a database". A previous run
    // may have moved the database but failed part-way through the directories, and a
    // one-shot guard would leave the old log file - the artefact containing message text -
    // sitting in the shared root forever. Each item below is checked individually, so
    // re-running is cheap and idempotent.

    logging::ensure_dir(&dest);

    for (from, to_rel) in [
        (root.join("eazyqq.db"), "eazyqq.db"),
        (root.join("eazyqq.db-shm"), "eazyqq.db-shm"),
        (root.join("eazyqq.db-wal"), "eazyqq.db-wal"),
        (root.join("logs"), "logs"),
        (root.join("group_files"), "group_files"),
        (root.join("diagnostics"), "diagnostics"),
    ] {
        if !from.exists() {
            continue;
        }
        let to = dest.join(to_rel);

        if from.is_dir() {
            // Merge rather than skip-on-exists. `ensure_layout` has usually already
            // created these directories empty, so a naive "destination exists" check
            // would leave the old contents behind - including the log file, which
            // contains message text and is the one thing that must not be left in a
            // shared location.
            match merge_dir(&from, &to) {
                Ok(n) if n > 0 => moved.push(format!("{}({})", to_rel, n)),
                Ok(_) => {}
                Err(e) => tracing::warn!("accounts: merging {} failed: {}", from.display(), e),
            }
            continue;
        }

        if to.exists() {
            continue;
        }
        match std::fs::rename(&from, &to) {
            Ok(_) => moved.push(to_rel.to_string()),
            Err(e) => tracing::warn!(
                "accounts: could not migrate {} -> {}: {}",
                from.display(),
                to.display(),
                e
            ),
        }
    }

    if !moved.is_empty() {
        tracing::info!(
            "accounts: migrated legacy data into account {} ({})",
            uin,
            moved.join(", ")
        );
    }
    Ok(moved)
}

/// Move every entry from `src` into `dst`.
///
/// A name collision is resolved by renaming rather than skipping: the account directory
/// already holds a live file under the same name (typically `eazyqq.log`, written since
/// isolation was introduced), but the older file must still leave the shared location -
/// it contains message text, and leaving it there is the exact leak this module exists to
/// prevent. Renaming keeps both.
///
/// Returns how many entries were moved.
fn merge_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<usize, String> {
    logging::ensure_dir(dst);
    let entries = std::fs::read_dir(src).map_err(|e| e.to_string())?;
    let mut moved = 0usize;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let mut target = dst.join(&name);

        if target.exists() && entry.path().is_file() {
            let stem = std::path::Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "legacy".to_string());
            let ext = std::path::Path::new(&name)
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            target = dst.join(format!("{}.legacy-{}{}", stem, legacy_stamp(), ext));
        }

        if target.exists() {
            // Directories that already exist are merged by the recursive call below only
            // when the destination is absent, so bail out here.
            continue;
        }

        if entry.path().is_dir() {
            if merge_dir(&entry.path(), &target).is_ok() {
                moved += 1;
            }
            continue;
        }

        if std::fs::rename(entry.path(), &target).is_ok() {
            moved += 1;
        }
    }

    // Only remove the source if nothing is left, so a partial move cannot lose data.
    if std::fs::read_dir(src)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(src);
    }
    Ok(moved)
}

/// Stable, sortable suffix for a migrated file that collided with a live one.
fn legacy_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_round_trips_through_camel_case_json() {
        // Regression: the struct once lacked rename_all, so a file containing
        // "lastAccount" deserialised to None and the app silently used the unbound
        // directory instead of the account's.
        let text = r#"{"lastAccount":"462564834","webviewCompatMode":true}"#;
        let b: Bootstrap = serde_json::from_str(text).expect("parse");
        assert_eq!(b.last_account.as_deref(), Some("462564834"));
        assert!(b.webview_compat_mode);

        let written = serde_json::to_string(&b).expect("serialise");
        assert!(
            written.contains("lastAccount") && written.contains("webviewCompatMode"),
            "must write camelCase so other tools can read it: {written}"
        );
    }

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        // The file is hand-editable; a typo must not break startup.
        let b: Bootstrap = serde_json::from_str("{}").expect("empty object is fine");
        assert!(b.last_account.is_none());
        assert!(!b.webview_compat_mode);

        let b: Bootstrap =
            serde_json::from_str(r#"{"lastAccount":"1","somethingElse":42}"#).expect("extra keys ok");
        assert_eq!(b.last_account.as_deref(), Some("1"));
    }

    #[test]
    fn merge_moves_files_and_renames_collisions() {
        let base = std::env::temp_dir().join("eazyqq_accounts_merge");
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        let dst = base.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        // A live file already occupies the destination name, as happens with eazyqq.log
        // written since isolation was introduced.
        std::fs::write(dst.join("eazyqq.log"), "live").unwrap();
        std::fs::write(src.join("eazyqq.log"), "legacy").unwrap();
        std::fs::write(src.join("other.log"), "other").unwrap();

        let moved = merge_dir(&src, &dst).unwrap();
        assert_eq!(moved, 2, "both files must leave the shared directory");

        // The live file is untouched and the legacy one survives under a new name - the
        // important part is that nothing containing message text is left behind.
        assert_eq!(std::fs::read_to_string(dst.join("eazyqq.log")).unwrap(), "live");
        assert_eq!(std::fs::read_to_string(dst.join("other.log")).unwrap(), "other");
        let legacy: Vec<_> = std::fs::read_dir(&dst)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("legacy"))
            .collect();
        assert_eq!(legacy.len(), 1, "the colliding file must be preserved under a new name");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn merge_leaves_nothing_behind_when_the_source_empties() {
        let base = std::env::temp_dir().join("eazyqq_accounts_merge2");
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        let dst = base.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), "x").unwrap();

        merge_dir(&src, &dst).unwrap();
        assert!(!src.exists(), "an emptied source directory should be removed");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn account_ids_cannot_escape_the_accounts_directory() {
        // Account ids arrive from the protocol side, so treat them as hostile.
        for hostile in ["../../etc", "..\\..\\windows", "a/b", "a\\b"] {
            let dir = account_dir(Some(hostile));
            let s = dir.to_string_lossy().replace('\\', "/");
            assert!(
                s.contains("/accounts/") && !s.contains(".."),
                "{hostile} produced {s}"
            );
        }
    }

    #[test]
    fn blank_account_falls_back_to_unbound() {
        assert!(account_dir(None).ends_with(UNBOUND));
        assert!(account_dir(Some("")).ends_with(UNBOUND));
        assert!(account_dir(Some("   ")).ends_with(UNBOUND));
    }

    #[test]
    fn normal_account_gets_its_own_directory() {
        let a = account_dir(Some("462564834"));
        let b = account_dir(Some("1739677116"));
        assert_ne!(a, b, "different accounts must not share a directory");
        assert!(a.ends_with("462564834"));
        assert!(b.ends_with("1739677116"));
    }

    #[test]
    fn path_is_keyed_by_machine_as_well_as_account() {
        // The workspace can be copied between machines; without the machine key, a restored
        // folder would present another machine's private data.
        let dir = account_dir(Some("462564834"));
        let s = dir.to_string_lossy().replace('\\', "/");
        assert!(
            s.contains(&format!("/accounts/{}/462564834", machine_id())),
            "expected a machine-keyed path, got {s}"
        );
    }

    #[test]
    fn machine_id_is_stable_and_non_empty() {
        let a = machine_id();
        let b = machine_id();
        assert_eq!(a, b, "machine id must not change between calls");
        assert!(!a.is_empty());
        assert!(a.len() <= 32, "machine id should stay short: {a}");
        assert!(
            a.chars().all(|c| c.is_ascii_hexdigit()),
            "machine id should be a plain hex token: {a}"
        );
    }

    #[test]
    fn fnv1a_matches_known_vectors() {
        // Standard FNV-1a 64 test vectors, so the directory key is reproducible.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn different_machines_would_produce_different_directories() {
        // Simulate by hashing two different identifiers directly.
        assert_ne!(fnv1a64(b"machine-a"), fnv1a64(b"machine-b"));
    }

    #[test]
    fn account_change_is_detected() {
        set_active(Some("111".to_string()));
        assert!(!account_changed("111"));
        assert!(account_changed("222"), "a different account must be reported");

        set_active(None);
        assert!(!account_changed("222"), "unknown active account cannot 'change'");
    }

    #[test]
    fn active_account_round_trips() {
        set_active(Some("999".to_string()));
        assert_eq!(active().as_deref(), Some("999"));
        assert!(active_data_dir().ends_with("999"));

        set_active(None);
        assert!(active().is_none());
        assert!(active_data_dir().ends_with(UNBOUND));
    }
}
