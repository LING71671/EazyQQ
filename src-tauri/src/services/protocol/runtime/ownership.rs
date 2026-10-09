//! Windows job objects own exactly the process tree launched for one protocol runtime.
//! A random persisted job name supports control from another CLI process after launcher exit.
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Child, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    job_name: String,
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    type Handle = *mut c_void;
    #[repr(C)]
    struct ThreadEntry {
        size: u32,
        usage: u32,
        id: u32,
        owner: u32,
        base_priority: i32,
        delta_priority: i32,
        flags: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Accounting {
        user_time: i64,
        kernel_time: i64,
        user_period: i64,
        kernel_period: i64,
        page_faults: u32,
        total_processes: u32,
        active_processes: u32,
        terminated_processes: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: Handle, name: *const u16) -> Handle;
        fn OpenJobObjectW(access: u32, inherit: i32, name: *const u16) -> Handle;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn QueryInformationJobObject(
            job: Handle,
            class: u32,
            info: Handle,
            size: u32,
            length: *mut u32,
        ) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn OpenThread(access: u32, inherit: i32, tid: u32) -> Handle;
        fn ResumeThread(thread: Handle) -> u32;
        fn GetLastError() -> u32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    pub(super) struct Guard(Handle);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn wide(name: &str) -> Vec<u16> {
        name.encode_utf16().chain(Some(0)).collect()
    }
    fn open(dir: &Path, access: u32) -> Option<Guard> {
        let receipt: Receipt =
            serde_json::from_slice(&std::fs::read(dir.join("eazyqq-process.json")).ok()?).ok()?;
        if !receipt.job_name.starts_with("Local\\EazyQQ-")
            || receipt.job_name.len() != "Local\\EazyQQ-".len() + 64
        {
            return None;
        }
        let handle = unsafe { OpenJobObjectW(access, 0, wide(&receipt.job_name).as_ptr()) };
        if handle.is_null() {
            None
        } else {
            Some(Guard(handle))
        }
    }
    pub fn running(dir: &Path) -> bool {
        let Some(job) = open(dir, 0x0004) else {
            return false;
        };
        let mut accounting = Accounting::default();
        unsafe {
            QueryInformationJobObject(
                job.0,
                1,
                &mut accounting as *mut _ as Handle,
                std::mem::size_of::<Accounting>() as u32,
                std::ptr::null_mut(),
            ) != 0
                && accounting.active_processes > 0
        }
    }
    fn resume(pid: u32) -> Result<(), String> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(0x00000004, 0) };
        if snapshot as isize == -1 {
            return Err("Cannot enumerate suspended launcher thread".into());
        }
        let snapshot = Guard(snapshot);
        let mut entry = ThreadEntry {
            size: std::mem::size_of::<ThreadEntry>() as u32,
            usage: 0,
            id: 0,
            owner: 0,
            base_priority: 0,
            delta_priority: 0,
            flags: 0,
        };
        let mut found = unsafe { Thread32First(snapshot.0, &mut entry) };
        while found != 0 {
            if entry.owner == pid {
                let thread = unsafe { OpenThread(0x0002, 0, entry.id) };
                if !thread.is_null() {
                    let thread = Guard(thread);
                    if unsafe { ResumeThread(thread.0) } != u32::MAX {
                        return Ok(());
                    }
                }
            }
            found = unsafe { Thread32Next(snapshot.0, &mut entry) };
        }
        Err("Could not resume the owned protocol launcher".into())
    }
    pub fn spawn(command: &mut Command, dir: &Path) -> Result<(Child, Guard), String> {
        use std::os::windows::process::CommandExt;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let name = format!(
            "Local\\EazyQQ-{}",
            random
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<String>()
        );
        let handle = unsafe { CreateJobObjectW(std::ptr::null_mut(), wide(&name).as_ptr()) };
        if handle.is_null() {
            return Err("Cannot create protocol ownership job".into());
        }
        let job = Guard(handle);
        if unsafe { GetLastError() } == 183 {
            return Err("Protocol ownership job already exists".into());
        }
        command.creation_flags(0x08000000 | 0x00000004);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let assigned = unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } != 0;
        let result = if assigned {
            crate::services::infra::persistence::write_json(
                &dir.join("eazyqq-process.json"),
                &Receipt { job_name: name },
            )
            .and_then(|_| resume(child.id()))
        } else {
            Err("Cannot assign protocol launcher to ownership job".into())
        };
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, job))
    }
    pub fn stop(dir: &Path) -> super::super::boot::BootOutcome {
        let Some(job) = open(dir, 0x0008 | 0x0004) else {
            return super::super::boot::BootOutcome {
                attempted: false,
                ok: true,
                detail: "No verified EazyQQ ownership job; other QQ sessions were preserved".into(),
            };
        };
        let ok = unsafe { TerminateJobObject(job.0, 0) } != 0;
        super::super::boot::BootOutcome {
            attempted: true,
            ok,
            detail: if ok {
                "Stopped only the selected account's owned protocol process tree"
            } else {
                "Could not stop the selected protocol ownership job"
            }
            .into(),
        }
    }
}

pub fn is_running(dir: &Path) -> bool {
    #[cfg(windows)]
    {
        win::running(dir)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        false
    }
}

#[derive(Serialize, Deserialize)]
struct LaunchPlan {
    request_id: String,
    executable: std::path::PathBuf,
    args: Vec<String>,
    environment: Vec<(String, String)>,
    workdir: std::path::PathBuf,
    output_log: Option<std::path::PathBuf>,
}
#[derive(Serialize, Deserialize)]
struct LaunchResult {
    request_id: String,
    ok: bool,
    detail: String,
}

pub fn spawn_owned(
    command: &mut Command,
    dir: &Path,
    output_log: Option<&Path>,
) -> Result<u32, String> {
    use crate::services::infra::filesystem::{physical_directory, physical_path, output_path};
    let physical_dir = physical_path(dir)?;
    let dir = physical_dir.as_path();
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let request_id = random
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    let plan = LaunchPlan {
        request_id: request_id.clone(),
        executable: physical_path(Path::new(command.get_program()))?,
        args: command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect(),
        environment: command
            .get_envs()
            .filter_map(|(k, v)| {
                v.map(|v| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect(),
        workdir: dir.into(),
        output_log: output_log.map(output_path).transpose()?,
    };
    crate::services::infra::persistence::write_json(&dir.join("eazyqq-launch.json"), &plan)?;
    let source = physical_path(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(&source).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let directory = physical_directory(&crate::services::accounts::data_root().join("runtimes/supervisors"))?;
    let executable = directory.join(format!("eazyqq-supervisor-{:x}.exe", hash.finalize()));
    if !executable.exists() {
        let temp = directory.join(format!("copy-{}.tmp", std::process::id()));
        std::fs::copy(&source, &temp).map_err(|e| e.to_string())?;
        if let Err(error) = std::fs::rename(&temp, &executable) {
            if !executable.exists() {
                return Err(error.to_string());
            }
            let _ = std::fs::remove_file(temp);
        }
    }
    // WMI starts the supervisor outside the caller's terminal/job lifetime.
    // This is required when the CLI is invoked by a host that terminates descendants.
    let command_line = format!(
        "\"{}\" --protocol-supervisor \"{}\"",
        executable.display(),
        dir.display()
    );
    let mut launcher = Command::new("powershell.exe");
    launcher.args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference = 'Stop'; $startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ShowWindow=[uint16]0}; $result = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine=$env:EAZYQQ_START_COMMANDLINE; CurrentDirectory=$env:EAZYQQ_START_DIRECTORY; ProcessStartupInformation=$startup}; if ($result.ReturnValue -ne 0) { [Console]::Error.WriteLine(('WMI process creation returned {0}' -f $result.ReturnValue)); exit 1 }; $result.ProcessId"])
        .env("EAZYQQ_START_COMMANDLINE", command_line).env("EAZYQQ_START_DIRECTORY", dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        launcher.creation_flags(0x08000000);
    }
    let output = launcher.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Cannot create a detached protocol supervisor: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| "Supervisor creation returned an invalid PID")?;
    for _ in 0..100 {
        if let Ok(bytes) = std::fs::read(dir.join("eazyqq-launch-result.json")) {
            if let Ok(result) = serde_json::from_slice::<LaunchResult>(&bytes) {
                if result.request_id == request_id {
                    return if result.ok {
                        Ok(pid)
                    } else {
                        Err(result.detail)
                    };
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = stop(dir);
    Err("Protocol supervisor did not confirm launch within 10 seconds".into())
}

/// Both distributed binaries enter this path before initializing GUI or CLI services.
pub fn run_supervisor_if_requested() -> Option<i32> {
    let mut args = std::env::args_os();
    let _ = args.next();
    let explicit = args
        .next()
        .filter(|arg| arg == "--protocol-supervisor")
        .and_then(|_| args.next());
    let dir = std::path::PathBuf::from(
        explicit.or_else(|| std::env::var_os("EAZYQQ_PROTOCOL_SUPERVISOR"))?,
    );
    std::env::remove_var("EAZYQQ_PROTOCOL_SUPERVISOR");
    let result = supervise(&dir);
    Some(if result.is_ok() { 0 } else { 1 })
}

fn supervise(dir: &Path) -> Result<(), String> {
    let plan: LaunchPlan = serde_json::from_slice(
        &std::fs::read(dir.join("eazyqq-launch.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if plan.workdir != dir
        || plan.executable.file_name().is_none_or(|name| {
            !name
                .to_string_lossy()
                .eq_ignore_ascii_case("NapCatWinBootMain.exe")
        })
    {
        return Err("Invalid protocol launch plan".into());
    }
    let mut command = Command::new(&plan.executable);
    command
        .args(&plan.args)
        .envs(plan.environment)
        .current_dir(dir);
    if let Some(log) = plan.output_log {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| e.to_string())?;
        command
            .stderr(file.try_clone().map_err(|e| e.to_string())?)
            .stdout(file);
    }
    #[cfg(windows)]
    {
        match win::spawn(&mut command, dir) {
            Ok((child, _job_guard)) => {
                crate::services::infra::persistence::write_json(
                    &dir.join("eazyqq-launch-result.json"),
                    &LaunchResult {
                        request_id: plan.request_id,
                        ok: true,
                        detail: "Owned protocol launched".into(),
                    },
                )?;
                drop(child);
                while is_running(dir) {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                Ok(())
            }
            Err(detail) => {
                crate::services::infra::persistence::write_json(
                    &dir.join("eazyqq-launch-result.json"),
                    &LaunchResult {
                        request_id: plan.request_id,
                        ok: false,
                        detail: detail.clone(),
                    },
                )?;
                Err(detail)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        Err("Protocol launch requires Windows".into())
    }
}

pub fn stop(dir: &Path) -> super::boot::BootOutcome {
    #[cfg(windows)]
    {
        win::stop(dir)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        super::boot::BootOutcome {
            attempted: false,
            ok: false,
            detail: "NapCat control is supported on Windows".into(),
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn owned_job_survives_launcher_handle_drop_and_stops_only_its_children() {
        let dir = std::env::temp_dir().join(format!("eazyqq-job-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]);
        let (child, _job_guard) = win::spawn(&mut command, &dir).unwrap();
        assert!(is_running(&dir));
        drop(child);
        assert!(is_running(&dir));
        assert!(stop(&dir).ok);
    }
    #[test]
    fn stale_pid_receipt_does_not_authorize_stopping_a_process() {
        let dir = std::env::temp_dir().join(format!("eazyqq-stale-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("eazyqq-process.json"),
            format!(
                r#"{{"pid":{},"created":0,"executable":"QQ.exe"}}"#,
                std::process::id()
            ),
        )
        .unwrap();
        assert!(!is_running(&dir));
        assert!(!stop(&dir).attempted);
    }
}
