param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'toolchain/msvc.ps1')
Enable-MsvcEnvironment
Push-Location (Join-Path $PSScriptRoot '../src-tauri')
try { & cargo @CargoArgs; $cargoExit = $LASTEXITCODE } finally { Pop-Location }
exit $cargoExit
