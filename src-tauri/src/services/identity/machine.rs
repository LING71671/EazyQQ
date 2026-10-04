use std::path::PathBuf;
use std::sync::OnceLock;

use crate::services::identity::bootstrap::{data_root, UNBOUND};

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

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

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Where a given account's data lives on this machine. `None` maps to the unbound directory.
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

/// Account ids come from the protocol side, so treat them as untrusted input.
pub fn sanitize_account(raw: &str) -> String {
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

/// Every other account directory on this machine, newest first.
pub fn other_account_dirs(except_uin: &str) -> Vec<PathBuf> {
    let machine = data_root().join("accounts").join(machine_id());
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();

    let Ok(entries) = std::fs::read_dir(&machine) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || path == account_dir(Some(except_uin)) {
            continue;
        }
        let stamp = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        dirs.push((stamp, path));
    }

    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    dirs.into_iter().map(|(_, p)| p).collect()
}
