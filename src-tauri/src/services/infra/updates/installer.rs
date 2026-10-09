//! Native installation handoff avoids shell encoding and terminal lifetime issues.
use super::download::PreparedUpdate;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    parent_id: u32,
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
    let result=(|| {
        wait_parent(request.parent_id)?;
        if std::fs::canonicalize(request.installer.parent().ok_or("Installer has no directory")?).map_err(|e|e.to_string())?
            != std::fs::canonicalize(root).map_err(|e|e.to_string())? {return Err("Installer is outside the verified update directory".into());}
        let bytes=std::fs::read(&request.installer).map_err(|e|e.to_string())?;
        if bytes.len() as u64 != request.size || format!("{:x}",Sha256::digest(&bytes)) != request.sha256 {return Err("Installer changed after verification".into());}
        let mut installer=std::process::Command::new(&request.installer);
        installer.args(["/P","/R"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            if let Some(directory)=&request.install_directory {
                installer.raw_arg(format!("/D={}",directory.display()));
            }
        }
        if let Some(root)=&request.root_override {installer.env("EAZYQQ_ROOT",root);}
        let status=installer.status().map_err(|e|e.to_string())?;
        if !status.success() {return Err(format!("Installer exited with code {:?}",status.code()));}
        Ok(())
    })();
    if let Err(error)=&result {
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
fn wait_parent(pid:u32)->Result<(),String> {
    use std::ffi::c_void;
    #[link(name="kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access:u32,inherit:i32,pid:u32)->*mut c_void;
        fn WaitForSingleObject(handle:*mut c_void,millis:u32)->u32;
        fn CloseHandle(handle:*mut c_void)->i32;
    }
    let handle=unsafe {OpenProcess(0x00100000,0,pid)};
    if handle.is_null() {
        let error=std::io::Error::last_os_error();
        return if error.raw_os_error()==Some(87) {Ok(())} else {Err(error.to_string())};
    }
    let result=unsafe {WaitForSingleObject(handle,90_000)};
    unsafe {CloseHandle(handle);}
    if result==0 {Ok(())} else {Err("The calling EazyQQ process did not exit before installation".into())}
}
#[cfg(not(windows))]
fn wait_parent(_:u32)->Result<(),String> {Err("Native installation is supported only on Windows".into())}

pub fn launch(prepared:&PreparedUpdate,desktop:bool)->Result<(),String> {
    #[cfg(not(windows))]
    {let _=(prepared,desktop);return Err("自动安装仅支持 Windows x64".into());}
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let root=prepared.path.parent().ok_or("安装包目录无效")?;
        let bytes=std::fs::read(&prepared.path).map_err(|e|e.to_string())?;
        if bytes.len() as u64 != prepared.size || format!("{:x}",Sha256::digest(&bytes))!=prepared.sha256 {return Err("安装包在校验后发生变化，已拒绝启动".into());}
        let source=std::env::current_exe().map_err(|e|e.to_string())?;
        let source_bytes=std::fs::read(&source).map_err(|e|e.to_string())?;
        let helper=root.join(format!("eazyqq-update-helper-{:x}.exe",Sha256::digest(source_bytes)));
        if !helper.exists() {std::fs::copy(&source,&helper).map_err(|e|e.to_string())?;}
        let request=root.join(format!("handoff-{}.json",std::process::id()));
        crate::services::infra::persistence::write_json(&request,&Request {
            parent_id:std::process::id(),installer:prepared.path.clone(),sha256:prepared.sha256.clone(),size:prepared.size,
            install_directory:if desktop {source.parent().map(Path::to_path_buf)} else {None},
            previous_executable:desktop.then_some(source),root_override:std::env::var("EAZYQQ_ROOT").ok(),
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
