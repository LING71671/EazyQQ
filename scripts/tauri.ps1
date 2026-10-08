param([Parameter(ValueFromRemainingArguments = $true)][string[]]$TauriArgs)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'toolchain/msvc.ps1')
Enable-MsvcEnvironment
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if ($TauriArgs[0] -eq 'build') {
        & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'build-bundle.ps1')
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    & pnpm exec tauri @TauriArgs
    $tauriExit = $LASTEXITCODE
} finally { Pop-Location }
exit $tauriExit
