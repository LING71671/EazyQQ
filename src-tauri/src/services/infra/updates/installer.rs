use super::download::PreparedUpdate;

// The helper waits for this EazyQQ process, never terminates QQ or another PID.
const HANDOFF: &str = r#"param([string]$RequestFile)
$ErrorActionPreference = 'Stop'
$request = Get-Content -LiteralPath $RequestFile -Raw | ConvertFrom-Json
try {
    Wait-Process -Id $request.parentId -Timeout 90 -ErrorAction SilentlyContinue
    if (Get-Process -Id $request.parentId -ErrorAction SilentlyContinue) { throw 'EazyQQ did not exit before installation' }
    if ((Get-FileHash -LiteralPath $request.installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $request.sha256) { throw 'Installer changed after verification' }
    $arguments = @('/P', '/R')
    if ($request.installDirectory) { $arguments += ('/D=' + $request.installDirectory) }
    $installerProcess = Start-Process -FilePath $request.installer -ArgumentList $arguments -Wait -PassThru
    if ($installerProcess.ExitCode -ne 0) { throw ('Installer exited with code ' + $installerProcess.ExitCode) }
} catch {
    $_ | Out-String | Set-Content -LiteralPath ($RequestFile + '.error.log') -Encoding utf8
    if ($request.previousExecutable -and (Test-Path -LiteralPath $request.previousExecutable)) {
        Start-Process -FilePath $request.previousExecutable
    }
    throw
}
"#;

pub fn launch(prepared: &PreparedUpdate, desktop: bool) -> Result<(), String> {
    #[cfg(not(windows))]
    { let _ = (prepared, desktop); return Err("自动安装仅支持 Windows x64".into()); }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use sha2::Digest;
        let installer = &prepared.path;
        let bytes = std::fs::read(installer).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != prepared.size || format!("{:x}", sha2::Sha256::digest(&bytes)) != prepared.sha256 {
            return Err("安装包在校验后发生变化，已拒绝启动".into());
        }
        let root = installer.parent().ok_or("安装包目录无效")?;
        let script = root.join("install-after-exit.ps1");
        std::fs::write(&script, HANDOFF).map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let request = root.join(format!("handoff-{}.json", std::process::id()));
        crate::services::infra::persistence::write_json(&request, &serde_json::json!({
            "parentId":std::process::id(), "installer":installer, "sha256":prepared.sha256,
            "installDirectory":if desktop { exe.parent() } else { None },
            "previousExecutable":if desktop { Some(&exe) } else { None }
        }))?;
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script).arg("-RequestFile").arg(&request)
            .creation_flags(0x08000000 | 0x00000008).spawn()
            .map_err(|e| format!("启动更新安装助手失败：{e}"))?;
        Ok(())
    }
}
