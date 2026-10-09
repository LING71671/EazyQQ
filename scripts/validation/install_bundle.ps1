$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Installer QA runs only on hosted CI, preserving the user installation registry' }
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$installer = Join-Path $projectRoot "src-tauri/target/release/bundle/nsis/EazyQQ_${version}_x64-setup.exe"
$target = Join-Path $projectRoot ('.test-runtime/installed QA-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $target 'resources/napcat'),(Join-Path $target 'EazyQQ_Data') -Force | Out-Null
$legacyDll = Join-Path $target 'resources/napcat/NapCatWinBootHook.dll'
Copy-Item -LiteralPath (Join-Path $env:SystemRoot 'System32/version.dll') -Destination $legacyDll
Set-Content -LiteralPath (Join-Path $target 'EazyQQ_Data/reinstall-sentinel.txt') -Value 'preserve account data'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class InstalledProtocolLock {
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr LoadLibrary(string name);
  [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
}
'@
$legacyHash = (Get-FileHash -LiteralPath $legacyDll).Hash
$legacyModule = [InstalledProtocolLock]::LoadLibrary($legacyDll)
if ($legacyModule -eq [IntPtr]::Zero) { throw 'Could not load legacy protocol lock fixture' }
try {
  $writeSucceeded = $false
  try { $file = [IO.File]::Open($legacyDll,[IO.FileMode]::Open,[IO.FileAccess]::Write,[IO.FileShare]::ReadWrite); $file.Dispose(); $writeSucceeded = $true } catch {}
  if ($writeSucceeded) { throw 'Legacy DLL lock fixture did not reproduce image write protection' }
$process = Start-Process -FilePath $installer -ArgumentList @('/S', ('/D=' + $target)) -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer exited with code $($process.ExitCode)" }
foreach ($file in @('eazyqq.exe','eazyqq_cli.exe',"resources/protocol/$version/NapCatWinBootMain.exe","resources/protocol/$version/NapCatWinBootHook.dll","resources/protocol/$version/napcat.mjs")) {
    if (-not (Test-Path -LiteralPath (Join-Path $target $file))) { throw "Installed bundle is missing $file" }
}
  $repeat = Start-Process -FilePath $installer -ArgumentList @('/S','/UPDATE',('/D=' + $target)) -WindowStyle Hidden -Wait -PassThru
  if ($repeat.ExitCode -ne 0) { throw 'Reinstallation while protocol was mapped failed' }
  if ((Get-FileHash -LiteralPath $legacyDll).Hash -ne $legacyHash) { throw 'Installer modified the running legacy protocol image' }
  if (-not (Test-Path -LiteralPath (Join-Path $target 'EazyQQ_Data/reinstall-sentinel.txt'))) { throw 'Reinstallation removed account data' }
} finally { [InstalledProtocolLock]::FreeLibrary($legacyModule) | Out-Null }
$cli = Join-Path $target 'eazyqq_cli.exe'
$actual = & $cli version --json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $actual.version -ne $version) { throw 'Installed CLI version does not match the bundle' }
"EAZYQQ_TEST_GUI=$(Join-Path $target 'eazyqq.exe')" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
"EAZYQQ_TEST_CLI=$cli" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
[ordered]@{installed=$true;reinstalledWithMappedProtocol=$true;accountDataPreserved=$true;version=$actual.version;target=$target} | ConvertTo-Json
