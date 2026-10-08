use std::path::Path;

use crate::services::identity::bootstrap::data_root;
use crate::services::identity::machine::{account_dir, other_account_dirs};
use crate::services::logging;
use crate::services::storage::db::Database;

fn move_file_or_copy(from: &Path, to: &Path) -> bool {
    if std::fs::rename(from, to).is_ok() {
        return true;
    }
    if std::fs::copy(from, to).is_ok() {
        let _ = std::fs::remove_file(from);
        return true;
    }
    false
}

/// Move pre-isolation data into the account directory that owns it.
pub fn migrate_legacy_if_needed(uin: &str) -> Result<Vec<String>, String> {
    let root = data_root();
    let dest = account_dir(Some(uin));
    let mut moved = Vec::new();

    logging::ensure_dir(&dest);

    // Sidecars belong to the source database, never to an existing destination DB.
    let move_database = root.join("eazyqq.db").exists() && !dest.join("eazyqq.db").exists();

    for (from, to_rel) in [
        (root.join("eazyqq.db"), "eazyqq.db"),
        (root.join("eazyqq.db-shm"), "eazyqq.db-shm"),
        (root.join("eazyqq.db-wal"), "eazyqq.db-wal"),
        (root.join("logs"), "logs"),
        (root.join("group_files"), "group_files"),
        (root.join("diagnostics"), "diagnostics"),
    ] {
        if to_rel.starts_with("eazyqq.db") && !move_database {
            continue;
        }
        if !from.exists() {
            continue;
        }
        let to = dest.join(to_rel);

        if from.is_dir() {
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
        if move_file_or_copy(&from, &to) {
            moved.push(to_rel.to_string());
        } else {
            tracing::warn!(
                "accounts: could not migrate {} -> {}",
                from.display(),
                to.display()
            );
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
pub fn merge_dir(src: &Path, dst: &Path) -> Result<usize, String> {
    logging::ensure_dir(dst);
    let entries = std::fs::read_dir(src).map_err(|e| e.to_string())?;
    let mut moved = 0usize;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let mut target = dst.join(&name);

        if target.exists() && entry.path().is_file() {
            let stem = Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "legacy".to_string());
            let ext = Path::new(&name)
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            target = dst.join(format!("{}.legacy-{}{}", stem, legacy_stamp(), ext));
        }

        if target.exists() {
            continue;
        }

        if entry.path().is_dir() {
            if merge_dir(&entry.path(), &target).is_ok() {
                moved += 1;
            }
            continue;
        }

        if move_file_or_copy(&entry.path(), &target) {
            moved += 1;
        }
    }

    if std::fs::read_dir(src)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(src);
    }
    Ok(moved)
}

/// Stable, sortable suffix for a migrated file that collided with a live one.
pub fn legacy_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

/// Bring artefacts that NapCat writes into the shared `napcat/` directory under the account.
pub fn sweep_napcat_artifacts(napcat_dir: &Path, uin: &str) -> Vec<String> {
    let mut moved = Vec::new();

    let logs_src = napcat_dir.join("logs");
    if logs_src.is_dir() {
        let dest = account_dir(Some(uin)).join("napcat_logs");
        match merge_dir(&logs_src, &dest) {
            Ok(n) if n > 0 => {
                moved.push(format!("napcat_logs({})", n));
                tracing::info!(
                    "accounts: moved {} NapCat log file(s) into account {} (contain message text)",
                    n,
                    uin
                );
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("accounts: could not sweep NapCat logs: {}", e),
        }
    }

    let qr = napcat_dir.join("cache").join("qrcode.png");
    if qr.is_file() {
        match std::fs::remove_file(&qr) {
            Ok(_) => {
                moved.push("cache/qrcode.png".to_string());
                tracing::info!("accounts: removed the login QR code from napcat/cache");
            }
            Err(e) => tracing::debug!("accounts: could not remove {}: {}", qr.display(), e),
        }
    }

    moved
}

/// Move data written before any account was known into the account that now owns it.
pub fn migrate_unbound_if_needed(uin: &str) -> Vec<String> {
    let unbound = account_dir(None);
    let dest = account_dir(Some(uin));
    let mut moved = Vec::new();

    if !unbound.exists() || unbound == dest {
        return moved;
    }

    logging::ensure_dir(&dest);

    let move_database = unbound.join("eazyqq.db").exists() && !dest.join("eazyqq.db").exists();

    for (from, to_rel) in [
        (unbound.join("eazyqq.db"), "eazyqq.db"),
        (unbound.join("eazyqq.db-shm"), "eazyqq.db-shm"),
        (unbound.join("eazyqq.db-wal"), "eazyqq.db-wal"),
        (unbound.join("logs"), "logs"),
        (unbound.join("group_files"), "group_files"),
        (unbound.join("diagnostics"), "diagnostics"),
        (unbound.join("napcat_logs"), "napcat_logs"),
    ] {
        if to_rel.starts_with("eazyqq.db") && !move_database {
            continue;
        }
        if !from.exists() {
            continue;
        }
        let to = dest.join(to_rel);

        if from.is_dir() {
            if let Ok(n) = merge_dir(&from, &to) {
                if n > 0 {
                    moved.push(format!("{}({})", to_rel, n));
                }
            }
            continue;
        }

        if to.exists() {
            continue;
        }
        if move_file_or_copy(&from, &to) {
            moved.push(to_rel.to_string());
        }
    }

    if !moved.is_empty() {
        tracing::info!(
            "accounts: moved pre-login data into account {} ({})",
            uin,
            moved.join(", ")
        );
        if std::fs::read_dir(&unbound)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(&unbound);
        }
    }
    moved
}

/// Give a brand-new account the settings that belong to the installation.
pub fn seed_settings_if_missing(db: &Database, uin: &str) -> Option<String> {
    if matches!(db.get_setting("app_config"), Ok(Some(_))) {
        return None;
    }

    let mut sources = vec![account_dir(None)];
    sources.extend(other_account_dirs(uin));

    for dir in sources {
        let path = dir.join("eazyqq.db");
        if !path.exists() {
            continue;
        }
        let Ok(conn) = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) else {
            continue;
        };
        let Ok(value) = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'app_config'",
            [],
            |row| row.get::<_, String>(0),
        ) else {
            continue;
        };

        if db.set_setting("app_config", &value).is_ok() {
            return Some(format!(
                "inherited installation settings from {}",
                dir.display()
            ));
        }
    }
    None
}
