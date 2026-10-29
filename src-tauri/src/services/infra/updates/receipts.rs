//! Durable update evidence. History queries never create or repair files.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::services::infra::persistence::{write_json, FileLock};

const LIMIT: usize = 100;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase { Checking, Checked, Downloading, Verifying, Ready, Handoff, WaitingParent, Installing, InstallerSucceeded, PendingVerification, Deployed, Succeeded, Failed, Aborted }
impl Phase {
    fn terminal(&self) -> bool { matches!(self, Self::Checked | Self::Deployed | Self::Succeeded | Self::Failed | Self::Aborted) }
    fn awaiting_verification(&self) -> bool { matches!(self, Self::InstallerSucceeded | Self::PendingVerification) }
    fn accepts(&self, next: &Self) -> bool {
        if self == next { return true; }
        if matches!(next, Self::Failed | Self::Aborted) { return !self.terminal() && !self.awaiting_verification(); }
        match self {
            Self::Checking => matches!(next, Self::Checked | Self::Downloading | Self::Verifying | Self::Ready),
            Self::Downloading => matches!(next, Self::Verifying),
            Self::Verifying => matches!(next, Self::Downloading | Self::Ready | Self::Installing),
            Self::Ready => matches!(next, Self::Handoff),
            Self::Handoff => matches!(next, Self::WaitingParent | Self::Installing),
            Self::WaitingParent => matches!(next, Self::Verifying),
            Self::Installing => matches!(next, Self::InstallerSucceeded | Self::PendingVerification | Self::Deployed),
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptEvent { pub phase: Phase, pub at_ms: u64 }
fn install_operation() -> String { "install".into() }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub component: String,
    #[serde(default = "install_operation")]
    pub operation: String,
    pub target_version: Option<String>,
    pub phase: Phase,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub installer_exit_code: Option<i32>,
    pub error: Option<String>,
    #[serde(default)]
    pub verified_version: Option<String>,
    #[serde(default)]
    pub events: Vec<ReceiptEvent>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerificationTicket { directory: PathBuf, target_version: String, #[serde(default)] observed_at_ms: Option<u64> }
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }
fn valid_id(id: &str) -> bool { id.len() == 32 && id.bytes().all(|value| value.is_ascii_hexdigit()) }
fn regular_directory(path: &Path) -> Result<(), String> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err("Update evidence directory is not a regular directory".into()); }
    }
    Ok(())
}
#[derive(Clone)]
pub struct Journal { root: PathBuf, id: String }
impl Journal {
    fn path(&self) -> PathBuf { self.root.join("receipts").join(format!("{}.json", self.id)) }
    fn ticket(&self) -> PathBuf { self.root.join("verifications").join(format!("{}.json", self.id)) }
    fn lock(&self) -> Result<FileLock, String> {
        regular_directory(&self.root.join("receipts"))?;
        FileLock::acquire(&self.root.join("receipts.lock"))
    }
    pub fn begin(root: &Path, target_version: Option<String>) -> Result<(Self, UpdateReceipt), String> { Self::begin_for(root, "app", "install", target_version, Phase::Handoff) }
    pub fn begin_for(root: &Path, component: &str, operation: &str, target_version: Option<String>, phase: Phase) -> Result<(Self, UpdateReceipt), String> {
        if !matches!(component, "app" | "napcat") || !matches!(operation, "check" | "install") { return Err("Invalid update operation".into()); }
        let mut bytes = [0u8; 16]; getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
        let journal = Self { root: root.into(), id: bytes.iter().map(|byte| format!("{byte:02x}")).collect() };
        let _lock = journal.lock()?; prune(root, LIMIT - 1)?;
        let time = now();
        let receipt = UpdateReceipt { schema_version: 1, operation_id: journal.id.clone(), component: component.into(), operation: operation.into(), target_version, phase: phase.clone(), started_at_ms: time, updated_at_ms: time, installer_exit_code: None, error: None, verified_version: None, events: vec![ReceiptEvent { phase, at_ms: time }] };
        write_json(&journal.path(), &receipt)?; Ok((journal, receipt))
    }
    pub fn resume(root: &Path, id: &str) -> Result<Self, String> {
        if !valid_id(id) { return Err("Invalid update operation identity".into()); }
        let journal = Self { root: root.into(), id: id.into() }; journal.read()?; Ok(journal)
    }
    pub fn id(&self) -> &str { &self.id }
    pub fn read(&self) -> Result<UpdateReceipt, String> {
        regular_directory(&self.root.join("receipts"))?;
        let path = self.path(); let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 256 * 1024 { return Err("Update receipt is not a bounded regular file".into()); }
        let value: UpdateReceipt = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if value.operation_id != self.id || value.schema_version != 1 || !matches!(value.component.as_str(), "app" | "napcat") || !matches!(value.operation.as_str(), "check" | "install") { return Err("Update receipt identity or schema is invalid".into()); }
        if value.started_at_ms == 0 || value.updated_at_ms < value.started_at_ms || value.updated_at_ms > 8_640_000_000_000_000
            || (value.phase.awaiting_verification() && value.installer_exit_code != Some(0))
            || (value.phase == Phase::Succeeded && (value.component != "app" || value.verified_version.is_none() || value.verified_version != value.target_version || value.installer_exit_code != Some(0)))
            || value.events.len() > 32 || value.events.iter().any(|event| event.at_ms < value.started_at_ms || event.at_ms > value.updated_at_ms) { return Err("Update receipt timestamp or success evidence is invalid".into()); }
        if value.events.windows(2).any(|events| events[0].at_ms > events[1].at_ms) || value.events.last().is_some_and(|event| event.phase != value.phase) { return Err("Update receipt timeline is inconsistent".into()); }
        Ok(value)
    }
    pub fn target(&self, version: &str) -> Result<(), String> {
        let _lock = self.lock()?; let mut value = self.read()?;
        if value.phase.terminal() { return Err("Update operation has ended".into()); }
        value.target_version = Some(version.trim_start_matches('v').into()); write_json(&self.path(), &value)
    }
    pub fn update(&self, phase: Phase, exit_code: Option<i32>, error: Option<&str>) -> Result<UpdateReceipt, String> {
        let _lock = self.lock()?; let mut value = self.read()?;
        if value.phase.terminal() || value.phase.awaiting_verification() { return Err("Update evidence cannot overwrite a terminal result or pending version verification".into()); }
        if !value.phase.accepts(&phase) { return Err("Update evidence cannot move to an unrelated or earlier stage".into()); }
        if phase.awaiting_verification() && (value.phase != Phase::Installing || exit_code != Some(0)) { return Err("Installer success requires the installing phase and a zero exit code".into()); }
        if phase == Phase::Succeeded { return Err("Version verification requires startup evidence".into()); }
        if value.phase == phase && error.is_none() { return Ok(value); }
        append(&mut value, phase, exit_code, error); write_json(&self.path(), &value)?; Ok(value)
    }
    pub fn prepare_verification(&self, directory: &Path) -> Result<(), String> {
        let _lock = self.lock()?; let value = self.read()?;
        let Some(target_version) = value.target_version else { return Ok(()); };
        regular_directory(&self.root.join("verifications"))?;
        write_json(&self.ticket(), &VerificationTicket { directory: crate::services::infra::filesystem::physical_path(directory)?, target_version, observed_at_ms: None })
    }
    pub fn confirm_observed(&self) -> Result<bool, String> {
        let _lock = self.lock()?; let mut value = self.read()?;
        if !value.phase.awaiting_verification() { return Ok(false); }
        let Some(ticket) = self.read_ticket()? else { return Ok(false); };
        if ticket.target_version != value.target_version.as_deref().unwrap_or_default() || !ticket.observed_at_ms.is_some_and(|time| time >= value.started_at_ms) { return Ok(false); }
        value.verified_version = Some(ticket.target_version);
        append(&mut value, Phase::Succeeded, Some(0), None); write_json(&self.path(), &value)?; Ok(true)
    }
    fn read_ticket(&self) -> Result<Option<VerificationTicket>, String> {
        regular_directory(&self.root.join("verifications"))?;
        let metadata = match std::fs::symlink_metadata(self.ticket()) { Ok(value) => value, Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None), Err(error) => return Err(error.to_string()) };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 64 * 1024 { return Err("Invalid update verification ticket".into()); }
        serde_json::from_slice(&std::fs::read(self.ticket()).map_err(|error| error.to_string())?).map(Some).map_err(|error| error.to_string())
    }
}
fn append(value: &mut UpdateReceipt, phase: Phase, exit_code: Option<i32>, error: Option<&str>) {
    value.phase = phase.clone(); value.updated_at_ms = now().max(value.updated_at_ms);
    if exit_code.is_some() { value.installer_exit_code = exit_code; }
    value.error = error.map(|detail| crate::services::diagnostics::redact(detail, &[]));
    value.events.push(ReceiptEvent { phase, at_ms: value.updated_at_ms }); if value.events.len() > 32 { value.events.remove(1); }
}
fn all(root: &Path) -> Result<Vec<UpdateReceipt>, String> {
    regular_directory(&root.join("receipts"))?;
    let entries = match std::fs::read_dir(root.join("receipts")) { Ok(entries) => entries, Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()), Err(error) => return Err(error.to_string()) };
    let mut values = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_none_or(|extension| extension != "json") { continue; }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()).filter(|id| valid_id(id)) else { continue; };
        values.push(Journal::resume(root, id)?.read()?);
    }
    values.sort_by(|left, right| right.started_at_ms.cmp(&left.started_at_ms).then_with(|| right.operation_id.cmp(&left.operation_id))); Ok(values)
}
pub fn history(root: &Path) -> Result<Vec<UpdateReceipt>, String> {
    // Opening an existing lock for reading avoids creating files during status queries.
    let lock = match std::fs::File::open(root.join("receipts.lock")) {
        Ok(file) => { fs2::FileExt::lock_shared(&file).map_err(|error| error.to_string())?; Some(file) },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let mut values = all(root)?; values.truncate(50); drop(lock); Ok(values)
}
fn prune(root: &Path, limit: usize) -> Result<(), String> {
    regular_directory(&root.join("verifications"))?;
    let mut values = all(root)?;
    while values.len() > limit {
        let Some(index) = values.iter().rposition(|value| value.phase.terminal()) else { return Err("更新记录包含100项未完成任务；请先核对未完成记录，未删除未知状态或用户数据".into()); };
        let value = values.remove(index);
        for path in [root.join("receipts").join(format!("{}.json", value.operation_id)), root.join("verifications").join(format!("{}.json", value.operation_id)), root.join(format!("handoff-{}.json", value.operation_id)), root.join(format!("handoff-{}.error.log", value.operation_id))] {
            if let Ok(metadata) = std::fs::symlink_metadata(&path) { if metadata.is_file() && !metadata.file_type().is_symlink() { std::fs::remove_file(path).map_err(|error| error.to_string())?; } }
        }
    }
    Ok(())
}
/// A dropped async task is a known cancellation; a killed process leaves unknown evidence intact.
pub struct Operation { journal: Journal, armed: bool }
impl Operation {
    pub fn begin(component: &str, operation: &str) -> Result<Self, String> { Self::at(&super::staging_root(), component, operation) }
    pub fn at(root: &Path, component: &str, operation: &str) -> Result<Self, String> { Ok(Self { journal: Journal::begin_for(root, component, operation, None, Phase::Checking)?.0, armed: true }) }
    pub fn journal(&self) -> &Journal { &self.journal }
    pub fn end<T>(&mut self, result: Result<T, String>, phase: Phase) -> Result<T, String> {
        self.armed = false;
        match result {
            Ok(value) => { if self.journal.read()?.phase != phase { self.journal.update(phase, None, None)?; } Ok(value) },
            Err(error) => { if !self.journal.read()?.phase.terminal() { self.journal.update(Phase::Failed, None, Some(&error))?; } Err(format!("更新 {}：{error}", self.journal.id())) },
        }
    }
    pub fn handoff(&mut self) { self.armed = false; }
}
impl Drop for Operation { fn drop(&mut self) { if self.armed { let _ = self.journal.update(Phase::Aborted, None, Some("更新任务在交接或部署完成前被取消；已保留现有程序与账号数据")); } } }
/// Only the running GUI from the recorded physical install directory proves its own version.
pub fn confirm_app_startup() -> Result<usize, String> {
    let executable = crate::services::infra::filesystem::physical_path(&std::env::current_exe().map_err(|error| error.to_string())?)?;
    if !executable.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("eazyqq.exe")) { return Ok(0); }
    confirm_startup_for(&super::staging_root(), env!("CARGO_PKG_VERSION"), executable.parent().ok_or("GUI executable has no directory")?)
}
fn confirm_startup_for(root: &Path, version: &str, directory: &Path) -> Result<usize, String> {
    let mut confirmed = 0;
    for value in all(root)?.into_iter().filter(|value| value.component == "app" && value.target_version.as_deref() == Some(version) && (value.phase == Phase::Installing || value.phase.awaiting_verification())) {
        let journal = Journal::resume(root, &value.operation_id)?;
        { let _lock = journal.lock()?; let Some(mut ticket) = journal.read_ticket()? else { continue; };
          if ticket.target_version != version || ticket.directory != directory { continue; }
          ticket.observed_at_ms = Some(now()); write_json(&journal.ticket(), &ticket)?; }
        if journal.confirm_observed()? { confirmed += 1; }
    }
    Ok(confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf { crate::services::logging::workspace_root().join(name) }
    #[test]
    fn installer_zero_is_pending_until_the_target_gui_starts() {
        let root = root("receipt-startup"); let directory = root.join("installed"); std::fs::create_dir_all(&directory).unwrap();
        let (journal, _) = Journal::begin(&root, Some("0.7.0".into())).unwrap(); journal.prepare_verification(&directory).unwrap();
        journal.update(Phase::Installing, None, None).unwrap(); journal.update(Phase::PendingVerification, Some(0), None).unwrap();
        assert!(!journal.confirm_observed().unwrap()); assert!(journal.update(Phase::Failed, None, Some("late error")).is_err());
        assert_eq!(confirm_startup_for(&root, "0.6.4", &directory).unwrap(), 0); assert_eq!(confirm_startup_for(&root, "0.7.0", &root.join("other")).unwrap(), 0);
        let physical = crate::services::infra::filesystem::physical_path(&directory).unwrap();
        assert_eq!(confirm_startup_for(&root, "0.7.0", &physical).unwrap(), 1); assert_eq!(journal.read().unwrap().phase, Phase::Succeeded);
    }
    #[test]
    fn early_gui_observation_waits_for_the_installer_exit() {
        let root = root("receipt-early-startup"); let directory = root.join("installed"); std::fs::create_dir_all(&directory).unwrap();
        let (journal, _) = Journal::begin(&root, Some("1.0.0".into())).unwrap(); journal.prepare_verification(&directory).unwrap(); journal.update(Phase::Installing, None, None).unwrap();
        let physical = crate::services::infra::filesystem::physical_path(&directory).unwrap();
        assert_eq!(confirm_startup_for(&root, "1.0.0", &physical).unwrap(), 0); assert_eq!(journal.read().unwrap().phase, Phase::Installing);
        journal.update(Phase::PendingVerification, Some(0), None).unwrap(); assert!(journal.confirm_observed().unwrap());
    }
    #[test]
    fn dropped_tasks_are_aborted_but_unfinished_records_are_not_guessed_dead() {
        let root = root("receipt-cancel"); let id;
        { let operation = Operation::at(&root, "napcat", "install").unwrap(); id = operation.journal().id().to_owned(); }
        assert_eq!(Journal::resume(&root, &id).unwrap().read().unwrap().phase, Phase::Aborted);
        let (unknown, _) = Journal::begin(&root, None).unwrap(); assert_eq!(history(&root).unwrap().len(), 2); assert_eq!(unknown.read().unwrap().phase, Phase::Handoff);
    }
    #[test]
    fn retention_only_removes_completed_owned_evidence_and_reads_are_read_only() {
        let root = root("receipt-retention"); assert!(history(&root).unwrap().is_empty()); assert!(!root.exists());
        std::fs::create_dir_all(&root).unwrap(); std::fs::write(root.join("personal.json"), "keep").unwrap(); let (pending, _) = Journal::begin(&root, None).unwrap();
        for _ in 0..110 { let (journal, _) = Journal::begin(&root, None).unwrap(); journal.update(Phase::Failed, None, Some("fixture")).unwrap(); }
        assert_eq!(all(&root).unwrap().len(), LIMIT); assert_eq!(history(&root).unwrap().len(), 50); assert!(pending.read().is_ok()); assert!(root.join("personal.json").exists());
    }
    #[test]
    fn legacy_receipts_remain_readable_without_claiming_verified_success() {
        let root = root("receipt-legacy"); let (journal, _) = Journal::begin(&root, None).unwrap();
        let mut old = serde_json::to_value(journal.read().unwrap()).unwrap(); for key in ["events", "operation", "verifiedVersion"] { old.as_object_mut().unwrap().remove(key); }
        old["phase"] = serde_json::json!("installer_succeeded"); old["installerExitCode"] = serde_json::json!(0); write_json(&journal.path(), &old).unwrap();
        assert_eq!(journal.read().unwrap().phase, Phase::InstallerSucceeded); assert!(!journal.confirm_observed().unwrap());
        old["operationId"] = serde_json::json!("wrong"); write_json(&journal.path(), &old).unwrap(); assert!(history(&root).is_err());
    }
    #[test]
    fn late_progress_cannot_erase_installation_evidence() {
        let (journal, _) = Journal::begin(&root("receipt-late-progress"), None).unwrap();
        journal.update(Phase::WaitingParent, None, None).unwrap(); journal.update(Phase::Verifying, None, None).unwrap(); journal.update(Phase::Installing, None, None).unwrap();
        assert!(journal.update(Phase::Checking, None, None).is_err()); assert!(journal.update(Phase::Downloading, None, None).is_err()); assert!(journal.update(Phase::PendingVerification, Some(9), None).is_err());
        assert_eq!(journal.read().unwrap().phase, Phase::Installing);
    }
}
