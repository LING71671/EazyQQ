//! Owned, short-lived inference directories. Persistent task sessions use a separate namespace.
use crate::services::infra::persistence::FileLock;
use std::path::{Path, PathBuf};

const MAX_REQUESTS: usize = 32;
const MAX_REQUEST_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

pub(crate) fn fresh_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "无法生成临时请求编号")?;
    Ok(format!(
        "inference_{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

pub(crate) struct RequestRuntime {
    account_dir: PathBuf,
    path: PathBuf,
    lease: Option<FileLock>,
    released: bool,
}

pub(crate) fn reject_link(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("AI 运行目录不可读取".into()),
    };
    if metadata.file_type().is_symlink() {
        return Err("AI 运行目录不能使用链接".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("AI 运行目录不能使用重解析点".into());
        }
    }
    Ok(())
}

fn inspect_tree(root: &Path) -> Result<u64, String> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut count = 0usize;
    let mut bytes = 0u64;
    while let Some((path, depth)) = pending.pop() {
        count += 1;
        if count > MAX_ENTRIES || depth > 32 {
            return Err("单次 AI 请求目录超过 10000 个条目或 32 层目录限制".into());
        }
        reject_link(&path)?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| "AI 请求缓存不可读取")?;
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).map_err(|_| "AI 请求缓存目录不可读取")?
            {
                pending.push((
                    entry.map_err(|_| "AI 请求缓存条目不可读取")?.path(),
                    depth + 1,
                ));
            }
        } else {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or("AI 请求缓存容量无效")?;
        }
    }
    Ok(bytes)
}

pub(crate) fn check_tree_budget(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    if inspect_tree(root)? > MAX_REQUEST_BYTES {
        return Err("单次 AI 会话缓存超过 128 MiB，结果未接受".into());
    }
    Ok(())
}

impl RequestRuntime {
    pub(crate) fn create(account_dir: &Path, id: &str) -> Result<Self, String> {
        let valid_suffix = id.rsplit_once('_').is_some_and(|(_, suffix)| {
            suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        if !valid_suffix
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("临时请求必须使用新生成的随机标识".into());
        }
        let runtime = account_dir.join("ai-runtime");
        let requests = runtime.join("requests");
        let path = requests.join(id);
        let admission_path = runtime.join("request-admission.lock");
        for ancestor in [
            account_dir,
            runtime.as_path(),
            requests.as_path(),
            path.as_path(),
            admission_path.as_path(),
        ] {
            reject_link(ancestor)?;
        }
        std::fs::create_dir_all(&requests).map_err(|_| "无法创建账号的临时 AI 目录")?;
        let account_dir = std::fs::canonicalize(account_dir).map_err(|_| "账号 AI 目录不可解析")?;
        let resolved = std::fs::canonicalize(&requests).map_err(|_| "临时 AI 目录不可解析")?;
        if resolved != account_dir.join("ai-runtime").join("requests") {
            return Err("临时 AI 目录不属于当前账号".into());
        }
        let _admission = FileLock::acquire(&admission_path)?;
        let count = std::fs::read_dir(&requests)
            .map_err(|_| "临时 AI 目录不可读取")?
            .map(|entry| entry.map_err(|_| "临时 AI 目录条目不可读取"))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|entry| {
                entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_dir() || kind.is_symlink())
            })
            .count();
        if count >= MAX_REQUESTS {
            return Err(format!("当前账号已保留 32 个临时 AI 请求目录；请结束运行中的请求，或在确认请求已停止后清理已中断缓存：{}",requests.display()));
        }
        std::fs::create_dir(&path)
            .map_err(|_| "临时 AI 请求目录已存在或不可创建，拒绝复用旧会话")?;
        let lease = match FileLock::try_acquire(&path.join("request.lock")) {
            Ok(lease) => lease,
            Err(error) => {
                let _ = std::fs::remove_file(path.join("request.lock"));
                let _ = std::fs::remove_dir(&path);
                return Err(error);
            }
        };
        Ok(Self {
            account_dir,
            path,
            lease: Some(lease),
            released: false,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn check_budget(&self) -> Result<(), String> {
        check_tree_budget(&self.path)
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if self.released {
            return Ok(());
        }
        self.released = true;
        let expected_parent = self.account_dir.join("ai-runtime").join("requests");
        let admission = self
            .account_dir
            .join("ai-runtime")
            .join("request-admission.lock");
        for path in [
            &self.account_dir,
            &self.account_dir.join("ai-runtime"),
            &expected_parent,
            &self.path,
            &admission,
        ] {
            reject_link(path)?;
        }
        let _admission = FileLock::acquire(&admission)?;
        let resolved = std::fs::canonicalize(&self.path)
            .map_err(|_| "本次 AI 请求缓存不可解析，未清理其他文件")?;
        if resolved.parent() != Some(expected_parent.as_path()) {
            return Err("本次 AI 请求目录物理位置变化，未清理文件".into());
        }
        inspect_tree(&self.path)?;
        drop(self.lease.take());
        std::fs::remove_dir_all(&self.path)
            .map_err(|_| "本次 AI 请求缓存仍被占用，已保留供手动清理".into())
    }

    pub(crate) fn release(mut self) -> Result<(), String> {
        self.cleanup()
    }
}

impl Drop for RequestRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            tracing::warn!(path=%self.path.display(), %error, "Owned AI request cache was retained");
        }
    }
}

#[cfg(test)]
#[path = "request_runtime/tests.rs"]
mod tests;
