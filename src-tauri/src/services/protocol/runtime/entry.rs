//! Electron joins package.main to the app directory, including on Windows.
use crate::services::infra::filesystem::{physical_directory, physical_path};
use std::path::{Path, PathBuf};

fn same_root(left: &Path, right: &Path) -> bool {
    let left = crate::services::infra::filesystem::process_path(left).ok();
    let right = crate::services::infra::filesystem::process_path(right).ok();
    left.as_ref()
        .and_then(|path| path.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .is_some_and(|left| {
            Some(left)
                == right
                    .as_ref()
                    .and_then(|path| path.components().next())
                    .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        })
}

fn relative(base: &Path, target: &Path) -> Result<String, String> {
    if !same_root(base, target) {
        return Err("Loader bridge is on the wrong volume".into());
    }
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    let common = base
        .iter()
        .zip(&target)
        .take_while(|(a, b)| {
            a.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
        })
        .count();
    let mut path = PathBuf::new();
    for _ in common..base.len() {
        path.push("..");
    }
    for component in &target[common..] {
        path.push(component.as_os_str());
    }
    Ok(path.to_string_lossy().replace('\\', "/"))
}

pub fn package_main(app_dir: &Path, loader: &Path) -> Result<String, String> {
    let app = physical_path(app_dir)?;
    let loader = physical_path(loader)?;
    if !loader.is_file() {
        return Err(format!("Private loader is missing: {}", loader.display()));
    }
    if same_root(&app, &loader) {
        return relative(&app, &loader);
    }
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(local).join("EazyQQ/loader-bridges"));
        }
        roots.push(crate::services::accounts::data_root().join("loader-bridges"));
        let drive = app
            .components()
            .next()
            .ok_or("QQ path has no drive")?
            .as_os_str();
        roots.push(PathBuf::from(drive).join("\\EazyQQ_Runtime/loader-bridges"));
        return bridge_main(&app, &loader, roots);
    }
    #[cfg(not(windows))]
    {
        Err("Cross-volume loaders require Windows directory junctions".into())
    }
}

#[cfg(windows)]
fn bridge_main(app: &Path, loader: &Path, roots: Vec<PathBuf>) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut selected = None;
    let mut errors = Vec::new();
    for candidate in roots.into_iter().filter(|root| same_root(&app, root)) {
        match physical_directory(&candidate) {
            Ok(root) if same_root(&app, &root) => {
                selected = Some(root);
                break;
            }
            Ok(_) => errors.push("Physical bridge directory is on another volume".to_string()),
            Err(error) => errors.push(error),
        }
    }
    let root = selected.ok_or_else(|| {
        format!(
            "Cannot locate a same-volume physical loader bridge: {}",
            errors.join("; ")
        )
    })?;
    let id = format!(
        "{:x}",
        Sha256::digest(loader.parent().unwrap().to_string_lossy().as_bytes())
    );
    let bridge = root.join(&id[..32]);
    let _lock = crate::services::infra::persistence::FileLock::acquire(
        &root.join(format!("{}.lock", &id[..32])),
    )?;
    junction(&bridge, loader.parent().unwrap())?;
    relative(app, &bridge.join(loader.file_name().unwrap()))
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    type Handle = *mut c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            disposition: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
        fn DeviceIoControl(
            handle: Handle,
            code: u32,
            input: *const c_void,
            input_len: u32,
            output: *mut c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
    if link.exists() {
        if std::fs::canonicalize(link).map_err(|e| e.to_string())? == target {
            return Ok(());
        }
        return Err(format!(
            "Loader bridge is occupied by another directory: {}",
            link.display()
        ));
    }
    std::fs::create_dir_all(link.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::create_dir(link).map_err(|e| e.to_string())?;
    let result = (|| {
        let path: Vec<u16> = link.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                0x40000000,
                7,
                std::ptr::null(),
                3,
                0x02200000,
                std::ptr::null_mut(),
            )
        };
        if handle as isize == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let printable = target.to_string_lossy();
        let plain = printable.strip_prefix(r"\\?\").unwrap_or(&printable);
        let substitute: Vec<u16> = format!(r"\??\{}", plain).encode_utf16().collect();
        let print: Vec<u16> = plain.encode_utf16().collect();
        let length = 8 + (substitute.len() + print.len() + 2) * 2;
        let mut data = Vec::new();
        data.extend_from_slice(&0xA0000003u32.to_le_bytes());
        data.extend_from_slice(&(length as u16).to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        for value in [
            0u16,
            (substitute.len() * 2) as u16,
            ((substitute.len() + 1) * 2) as u16,
            (print.len() * 2) as u16,
        ] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for value in substitute
            .into_iter()
            .chain(Some(0))
            .chain(print)
            .chain(Some(0))
        {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let mut returned = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                0x000900A4,
                data.as_ptr().cast(),
                data.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        let error = std::io::Error::last_os_error();
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 {
            return Err(error.to_string());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir(link);
    }
    result
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn detached_loader_reader() {
        let Some(request) = std::env::args_os().find(|arg| {
            Path::new(arg)
                .file_name()
                .is_some_and(|name| name == "loader-handoff.json")
        }) else {
            return;
        };
        let request = PathBuf::from(request);
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&request).unwrap()).unwrap();
        let output=std::process::Command::new("node")
            .args(["-e",r#"const fs=require('node:fs'),path=require('node:path');const entry=path.join(process.argv[1],process.argv[2]);console.log(JSON.stringify({resolved:require.resolve(entry),content:fs.readFileSync(entry,'utf8')}));"#])
            .arg(data["appDir"].as_str().unwrap()).arg(data["main"].as_str().unwrap())
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(request.with_extension("result.json"), output.stdout).unwrap();
    }

    #[test]
    fn detached_receiver_resolves_bridge_entry_with_its_own_filesystem_view() {
        use std::os::windows::process::CommandExt;
        let root = crate::services::logging::workspace_root().join("detached loader 中文");
        let app = root.join("qq/resources/app");
        let private = root.join("private");
        let physical = root.join("physical-cache");
        for dir in [&app, &private, &physical] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let loader = private.join("loadNapCat.cjs");
        std::fs::write(&loader, b"module.exports = 'private fixture';").unwrap();
        let logical = root.join("logical-cache");
        junction(&logical, &physical).unwrap();
        let app = physical_path(&app).unwrap();
        let loader = physical_path(&loader).unwrap();
        let main = bridge_main(&app, &loader, vec![logical.join("bridges")]).unwrap();
        assert!(!main.contains("logical-cache"));
        let request = physical_path(&root).unwrap().join("loader-handoff.json");
        std::fs::write(
            &request,
            serde_json::to_vec(&serde_json::json!({"appDir":app,"main":main})).unwrap(),
        )
        .unwrap();
        let executable = physical_path(&std::env::current_exe().unwrap()).unwrap();
        let command=format!("\"{}\" --exact services::protocol::entry::tests::detached_loader_reader \"{}\" --nocapture",executable.display(),request.display());
        let output=std::process::Command::new("powershell.exe")
            .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; $startup=New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ShowWindow=[uint16]0}; $result=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine=$env:EAZYQQ_FIXTURE_COMMAND;CurrentDirectory=$env:EAZYQQ_FIXTURE_DIRECTORY;ProcessStartupInformation=$startup}; if($result.ReturnValue -ne 0){throw $result.ReturnValue}"])
            .env("EAZYQQ_FIXTURE_COMMAND",command).env("EAZYQQ_FIXTURE_DIRECTORY",request.parent().unwrap())
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = request.with_extension("result.json");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !result.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let result: serde_json::Value = serde_json::from_slice(
            &std::fs::read(result)
                .expect("Detached Node receiver did not confirm entry resolution"),
        )
        .unwrap();
        assert_eq!(result["content"], "module.exports = 'private fixture';");
        assert_eq!(
            physical_path(Path::new(result["resolved"].as_str().unwrap())).unwrap(),
            loader
        );
    }

    #[test]
    fn package_entry_uses_physical_loader_and_normalizes_verbatim_drive() {
        let root = crate::services::logging::workspace_root().join("physical-entry");
        let app = root.join("qq/resources/app");
        let private = root.join("physical-private");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("loadNapCat.cjs"), b"private module").unwrap();
        let alias = root.join("logical-private");
        junction(&alias, &private).unwrap();
        let main = package_main(
            &std::fs::canonicalize(&app).unwrap(),
            &alias.join("loadNapCat.cjs"),
        )
        .unwrap();
        assert!(
            !main.contains("logical-private"),
            "Logical namespace leaked: {main}"
        );
        assert_eq!(std::fs::read(app.join(main)).unwrap(), b"private module");
    }

    #[test]
    fn bridge_parent_is_resolved_before_following_the_private_target() {
        let root = crate::services::logging::workspace_root().join("physical-bridge-entry");
        let app = root.join("qq/resources/app");
        let private = root.join("private 中文");
        let physical = root.join("physical-cache");
        for dir in [&app, &private, &physical] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let loader = private.join("loadNapCat.cjs");
        std::fs::write(&loader, b"private module").unwrap();
        let logical = root.join("logical-cache");
        junction(&logical, &physical).unwrap();
        let main = bridge_main(
            &physical_path(&app).unwrap(),
            &physical_path(&loader).unwrap(),
            vec![logical.join("bridges")],
        )
        .unwrap();
        assert!(
            main.contains("physical-cache"),
            "Physical bridge parent missing: {main}"
        );
        assert!(
            !main.contains("logical-cache"),
            "Logical namespace leaked: {main}"
        );
        assert_eq!(std::fs::read(app.join(main)).unwrap(), b"private module");
    }

    #[test]
    fn directory_bridge_reads_only_its_expected_private_loader() {
        let root = crate::services::logging::workspace_root().join("junction-entry");
        let target = root.join("private");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("loadNapCat.js"), b"private module").unwrap();
        let bridge = root.join("bridge");
        super::junction(&bridge, &target).unwrap();
        assert_eq!(
            std::fs::read(bridge.join("loadNapCat.js")).unwrap(),
            b"private module"
        );
        assert!(super::junction(&bridge, &root).is_err());
    }
}
