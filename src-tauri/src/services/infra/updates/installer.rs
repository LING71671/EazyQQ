//! Native installation handoff avoids shell encoding and terminal lifetime issues.
use super::download::PreparedUpdate;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use super::receipts::{Journal, Phase, UpdateReceipt};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    #[serde(default)]
    operation_id: Option<String>,
    #[serde(default)]
    telemetry_dsn: Option<String>,
    parent_id: u32,
    #[serde(default)]
    parent_started_at: Option<u64>,
    installer: PathBuf,
    sha256: String,
    size: u64,
    install_directory: Option<PathBuf>,
    previous_executable: Option<PathBuf>,
    root_override: Option<String>,
}

pub fn run_helper_if_requested() -> Option<i32> {
    let mut args=std::env::args_os();args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--app-update-helper")) { return None; }
    let result=args.next().ok_or_else(|| "Update helper request is missing".to_string())
        .and_then(|path|run_helper(Path::new(&path)));
    Some(if result.is_ok() {0} else {1})
}

pub(super) fn run_helper(request_path: &Path) -> Result<(),String> {
    let current=std::env::current_exe().map_err(|e|e.to_string())?;
    let root=current.parent().ok_or("Update helper has no directory")?;
    if std::fs::canonicalize(request_path.parent().ok_or("Request has no directory")?).map_err(|e|e.to_string())?
        != std::fs::canonicalize(root).map_err(|e|e.to_string())? {return Err("Update request is outside the helper directory".into());}
    let request:Request=serde_json::from_slice(&std::fs::read(request_path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let _telemetry = request.telemetry_dsn.as_deref().and_then(|dsn| crate::services::infra::telemetry::initialize_from_dsn("update-helper", dsn));
    let journal = request.operation_id.as_deref().map(|id| Journal::resume(root, id)).transpose()?;
    let mut exit_code = None;
    let mut aborted = false;
    let result=(|| {
        let _update = crate::services::infra::persistence::FileLock::try_acquire(&root.join("download.lock"))
            .map_err(|_| "另一个更新任务正在执行，未启动安装程序".to_string())?;
        if let Some(journal) = &journal { journal.update(Phase::WaitingParent, None, None)?; }
        if let Err(error) = wait_parent(request.parent_id, request.parent_started_at) { aborted = true; return Err(error); }
        if let Some(journal) = &journal { journal.update(Phase::Verifying, None, None)?; }
        if std::fs::canonicalize(request.installer.parent().ok_or("Installer has no directory")?).map_err(|e|e.to_string())?
            != std::fs::canonicalize(root).map_err(|e|e.to_string())? {return Err("Installer is outside the verified update directory".into());}
        let bytes=std::fs::read(&request.installer).map_err(|e|e.to_string())?;
        if bytes.len() as u64 != request.size || format!("{:x}",Sha256::digest(&bytes)) != request.sha256 {return Err("Installer changed after verification".into());}
        let mut installer=std::process::Command::new(&request.installer);
        installer.args(["/P","/R","/UPDATE"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            if let Some(directory)=&request.install_directory {
                installer.raw_arg(format!("/D={}",directory.display()));
            }
        }
        if let Some(root)=&request.root_override {installer.env("EAZYQQ_ROOT",root);}
        if let Some(journal) = &journal { journal.update(Phase::Installing, None, None)?; }
        let status=installer.status().map_err(|e|e.to_string())?;
        exit_code = status.code();
        if !status.success() {return Err(format!("安装程序未成功结束，退出码 {}",status.code().map(|value| value.to_string()).unwrap_or_else(|| "未提供".into())));}
        if let Some(journal) = &journal {
            journal.update(Phase::PendingVerification, exit_code, None)?;
            journal.confirm_observed()?;
        }
        Ok(())
    })();
    if let Err(error)=&result {
        if let Some(journal) = &journal { let _ = journal.update(if aborted { Phase::Aborted } else { Phase::Failed }, exit_code, Some(error)); }
        crate::services::infra::telemetry::capture_failure("update.installer", request.operation_id.as_deref(), exit_code);
        let _=std::fs::write(request_path.with_extension("error.log"),error);
        if let Some(previous)=&request.previous_executable {
            let mut app=std::process::Command::new(previous);
            if let Some(root)=&request.root_override {app.env("EAZYQQ_ROOT",root);}
            let _=app.spawn();
        }
    }
    result
}

#[cfg(windows)]
fn wait_parent(pid:u32, expected_started_at:Option<u64>)->Result<(),String> {
    use std::ffi::c_void;
    #[link(name="kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access:u32,inherit:i32,pid:u32)->*mut c_void;
        fn WaitForSingleObject(handle:*mut c_void,millis:u32)->u32;
        fn CloseHandle(handle:*mut c_void)->i32;
        fn GetProcessTimes(handle:*mut c_void,created:*mut u64,exited:*mut u64,kernel:*mut u64,user:*mut u64)->i32;
    }
    let handle=unsafe {OpenProcess(0x00100000 | 0x1000,0,pid)};
    if handle.is_null() {
        let error=std::io::Error::last_os_error();
        return if error.raw_os_error()==Some(87) {Ok(())} else {Err(error.to_string())};
    }
    if let Some(expected) = expected_started_at {
        let (mut created, mut exited, mut kernel, mut user) = (0, 0, 0, 0);
        let read = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
        if read == 0 { let error = std::io::Error::last_os_error(); unsafe { CloseHandle(handle); } return Err(format!("无法确认更新调用进程身份，安装未执行：{error}")); }
        if created != expected { unsafe { CloseHandle(handle); } return Ok(()); }
    }
    let result=unsafe {WaitForSingleObject(handle,90_000)};
    let wait_error = (result == u32::MAX).then(std::io::Error::last_os_error);
    unsafe {CloseHandle(handle);}
    match result {
        0 => Ok(()),
        258 => Err("调用更新的 EazyQQ 未在90秒内退出，安装未执行".into()),
        u32::MAX => Err(format!("等待调用程序失败：{}", wait_error.unwrap())),
        value => Err(format!("等待调用程序返回未知状态：{value}")),
    }
}
#[cfg(not(windows))]
fn wait_parent(_:u32,_:Option<u64>)->Result<(),String> {Err("Native installation is supported only on Windows".into())}

#[cfg(windows)]
fn caller_started_at() -> Result<u64, String> {
    use std::ffi::c_void;
    #[link(name="kernel32")]
    unsafe extern "system" { fn GetCurrentProcess()->*mut c_void; fn GetProcessTimes(handle:*mut c_void,created:*mut u64,exited:*mut u64,kernel:*mut u64,user:*mut u64)->i32; }
    let (mut created, mut exited, mut kernel, mut user) = (0, 0, 0, 0);
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) } == 0 { return Err(std::io::Error::last_os_error().to_string()); }
    Ok(created)
}

pub fn launch(prepared: &PreparedUpdate, desktop: bool) -> Result<UpdateReceipt, String> {
    // The package can disappear after prepare; its existing parent still locates the receipt.
    let parent = prepared.path.parent().ok_or("安装包目录无效")?;
    let root = crate::services::infra::filesystem::physical_path(parent)?;
    let journal = match &prepared.operation_id {
        Some(id) => { let journal = Journal::resume(&root, id)?; journal.update(Phase::Handoff, None, None)?; journal },
        None => Journal::begin(&root, prepared.target_version.clone())?.0,
    };
    if let Err(error) = launch_inner(prepared, desktop, &journal) {
        let _ = journal.update(Phase::Failed, None, Some(&error));
        return Err(format!("更新 {} 交接失败：{error}", journal.id()));
    }
    journal.read()
}

fn launch_inner(prepared:&PreparedUpdate,desktop:bool,journal:&Journal)->Result<(),String> {
    #[cfg(not(windows))]
    {let _=(prepared,desktop,journal);return Err("自动安装仅支持 Windows x64".into());}
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use crate::services::infra::filesystem::physical_path;
        let installer_path=physical_path(&prepared.path)?;
        let root=installer_path.parent().ok_or("安装包目录无效")?;
        let bytes=std::fs::read(&installer_path).map_err(|e|e.to_string())?;
        if bytes.len() as u64 != prepared.size || format!("{:x}",Sha256::digest(&bytes))!=prepared.sha256 {return Err("安装包在校验后发生变化，已拒绝启动".into());}
        let source=physical_path(&std::env::current_exe().map_err(|e|e.to_string())?)?;
        let install_directory = if desktop { source.parent().map(Path::to_path_buf) }
            else { source.parent().filter(|directory| directory.join("eazyqq.exe").is_file()).map(Path::to_path_buf) };
        if let Some(directory) = &install_directory { journal.prepare_verification(directory)?; }
        let source_bytes=std::fs::read(&source).map_err(|e|e.to_string())?;
        let helper=root.join(format!("eazyqq-update-helper-{:x}.exe",Sha256::digest(source_bytes)));
        if !helper.exists() {std::fs::copy(&source,&helper).map_err(|e|e.to_string())?;}
        let request=root.join(format!("handoff-{}.json",journal.id()));
        crate::services::infra::persistence::write_json(&request,&Request {
            operation_id: Some(journal.id().into()),
            telemetry_dsn: crate::services::infra::telemetry::helper_dsn(),
            parent_id:std::process::id(),parent_started_at:Some(caller_started_at()?),installer:installer_path.clone(),sha256:prepared.sha256.clone(),size:prepared.size,
            install_directory,
            previous_executable:desktop.then_some(source),root_override:std::env::var_os("EAZYQQ_ROOT")
                .map(|path|physical_path(Path::new(&path)).map(|p|p.to_string_lossy().into_owned())).transpose()?,
        })?;
        #[cfg(not(test))]
        let command_line=format!("\"{}\" --app-update-helper \"{}\"",helper.display(),request.display());
        #[cfg(test)]
        let command_line=format!("\"{}\" --exact services::infra::updates::tests::handoff_worker \"{}\" --nocapture",helper.display(),request.display());
        let output=std::process::Command::new("powershell.exe")
            .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; $startup=New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ShowWindow=[uint16]0}; $result=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine=$env:EAZYQQ_UPDATE_COMMAND;CurrentDirectory=$env:EAZYQQ_UPDATE_DIRECTORY;ProcessStartupInformation=$startup}; if($result.ReturnValue -ne 0){throw ('Installer helper creation failed: '+$result.ReturnValue)}; $result.ProcessId"])
            .env("EAZYQQ_UPDATE_COMMAND",command_line).env("EAZYQQ_UPDATE_DIRECTORY",root)
            .creation_flags(0x08000000).output().map_err(|e|e.to_string())?;
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim().parse::<u32>().is_err() {
            return Err(format!("更新安装助手未启动：{}",String::from_utf8_lossy(&output.stderr).trim()));
        }
        Ok(())
    }
}
