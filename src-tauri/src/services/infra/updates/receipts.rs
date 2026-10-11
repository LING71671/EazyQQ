//! Durable installer evidence shared by the caller and detached helper.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::services::infra::persistence::{write_json, FileLock};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase { Handoff, WaitingParent, Verifying, Installing, InstallerSucceeded, Failed }

impl Phase {
    fn rank(&self) -> u8 {
        match self { Self::Handoff => 0, Self::WaitingParent => 1, Self::Verifying => 2, Self::Installing => 3, Self::InstallerSucceeded | Self::Failed => 4 }
    }
    fn terminal(&self) -> bool { matches!(self, Self::InstallerSucceeded | Self::Failed) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub component: String,
    pub target_version: Option<String>,
    pub phase: Phase,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub installer_exit_code: Option<i32>,
    pub error: Option<String>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
fn valid_id(id: &str) -> bool { id.len() == 32 && id.bytes().all(|value| value.is_ascii_hexdigit()) }

pub struct Journal { path: PathBuf, id: String }
impl Journal {
    pub fn begin(root: &Path, target_version: Option<String>) -> Result<(Self, UpdateReceipt), String> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
        let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let journal = Self { path: root.join("receipts").join(format!("{id}.json")), id: id.clone() };
        let time = now();
        let receipt = UpdateReceipt { schema_version: 1, operation_id: id, component: "app".into(), target_version, phase: Phase::Handoff, started_at_ms: time, updated_at_ms: time, installer_exit_code: None, error: None };
        write_json(&journal.path, &receipt)?;
        Ok((journal, receipt))
    }
    pub fn resume(root: &Path, id: &str) -> Result<Self, String> {
        if !valid_id(id) { return Err("Invalid update operation identity".into()); }
        let journal = Self { path: root.join("receipts").join(format!("{id}.json")), id: id.into() };
        journal.read()?;
        Ok(journal)
    }
    pub fn id(&self) -> &str { &self.id }
    fn read(&self) -> Result<UpdateReceipt, String> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() > 256 * 1024 { return Err("Update receipt is not a bounded regular file".into()); }
        let value: UpdateReceipt = serde_json::from_slice(&std::fs::read(&self.path).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        if value.operation_id != self.id || value.schema_version != 1 || value.component != "app" {
            return Err("Update receipt identity or schema is invalid".into());
        }
        if value.started_at_ms == 0 || value.updated_at_ms < value.started_at_ms || value.updated_at_ms > 8_640_000_000_000_000
            || (value.phase == Phase::InstallerSucceeded && value.installer_exit_code != Some(0)) {
            return Err("Update receipt timestamp or success evidence is invalid".into());
        }
        Ok(value)
    }
    pub fn update(&self, phase: Phase, exit_code: Option<i32>, error: Option<&str>) -> Result<UpdateReceipt, String> {
        let _lock = FileLock::acquire(&self.path.with_extension("lock"))?;
        let mut value = self.read()?;
        if value.phase.terminal() || phase.rank() < value.phase.rank() {
            return Err("Update evidence cannot overwrite a terminal or later phase".into());
        }
        if phase == Phase::InstallerSucceeded && (value.phase != Phase::Installing || exit_code != Some(0)) {
            return Err("Installer success requires the installing phase and a zero exit code".into());
        }
        value.phase = phase;
        value.updated_at_ms = now().max(value.updated_at_ms);
        value.installer_exit_code = exit_code;
        value.error = error.map(|detail| crate::services::diagnostics::redact(detail, &[]));
        write_json(&self.path, &value)?;
        Ok(value)
    }
}

pub fn history(root: &Path) -> Result<Vec<UpdateReceipt>, String> {
    let directory = root.join("receipts");
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut values = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") { continue; }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()).filter(|id| valid_id(id)) else { continue; };
        values.push(Journal::resume(root, id)?.read()?);
    }
    values.sort_by(|left, right| right.started_at_ms.cmp(&left.started_at_ms).then_with(|| right.operation_id.cmp(&left.operation_id)));
    values.truncate(50);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf { crate::services::logging::workspace_root().join(name) }
    #[test]
    fn caller_and_helper_share_durable_result_and_terminal_evidence() {
        let root = root("receipt-shared");
        let (caller, initial) = Journal::begin(&root, Some("0.7.0".into())).unwrap();
        let helper = Journal::resume(&root, caller.id()).unwrap();
        helper.update(Phase::WaitingParent, None, None).unwrap();
        helper.update(Phase::Installing, None, None).unwrap();
        helper.update(Phase::InstallerSucceeded, Some(0), None).unwrap();
        assert!(caller.update(Phase::Handoff, None, None).is_err());
        assert!(caller.update(Phase::Failed, None, Some("late failure")).is_err());
        let values = history(&root).unwrap();
        assert_eq!(values[0].operation_id, initial.operation_id);
        assert_eq!(values[0].phase, Phase::InstallerSucceeded);
        assert_eq!(values[0].installer_exit_code, Some(0));
    }
    #[test]
    fn failure_is_separate_from_handoff_and_retry_has_a_new_identity() {
        let root = root("receipt-failure-retry");
        let (first, _) = Journal::begin(&root, None).unwrap();
        first.update(Phase::Failed, Some(9), Some("installation refused")).unwrap();
        let (second, _) = Journal::begin(&root, None).unwrap();
        assert_ne!(first.id(), second.id());
        let values = history(&root).unwrap();
        assert_eq!(values.len(), 2);
        assert!(values.iter().any(|value| value.phase == Phase::Failed && value.installer_exit_code == Some(9)));
    }
    #[test]
    fn success_requires_installer_exit_evidence() {
        let (journal, _) = Journal::begin(&root("receipt-proof"), None).unwrap();
        assert!(journal.update(Phase::InstallerSucceeded, Some(0), None).is_err());
        journal.update(Phase::Installing, None, None).unwrap();
        assert!(journal.update(Phase::InstallerSucceeded, None, None).is_err());
        assert!(journal.update(Phase::InstallerSucceeded, Some(9), None).is_err());
        journal.update(Phase::InstallerSucceeded, Some(0), None).unwrap();
    }
    #[test]
    fn path_traversal_and_corrupt_identity_are_rejected() {
        let root = root("receipt-invalid");
        assert!(Journal::resume(&root, "../outside").is_err());
        let (journal, mut receipt) = Journal::begin(&root, None).unwrap();
        receipt.operation_id = "invalid identity".into();
        write_json(&journal.path, &receipt).unwrap();
        assert!(history(&root).is_err());
    }
    #[test]
    fn empty_history_is_read_only_and_results_are_bounded() {
        let root = root("receipt-bounded");
        assert!(history(&root).unwrap().is_empty());
        assert!(!root.exists());
        for _ in 0..55 { Journal::begin(&root, None).unwrap(); }
        assert_eq!(history(&root).unwrap().len(), 50);
        assert_eq!(std::fs::read_dir(root.join("receipts")).unwrap().count(), 55);
    }
}
