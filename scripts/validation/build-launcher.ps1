param([Parameter(Mandatory = $true)][string]$Target)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../.test-runtime'))
$taskTarget = [IO.Path]::GetFullPath($Target)
if (-not $taskTarget.StartsWith($taskRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Fixture output must remain inside .test-runtime'
}
. (Join-Path $PSScriptRoot '../toolchain/msvc.ps1')
Enable-MsvcEnvironment
Push-Location (Split-Path $taskTarget)
try {
    & cl.exe /nologo /MT /EHsc (Join-Path $PSScriptRoot 'fixtures/protocol_launcher.cpp') "/Fe:$taskTarget"
    if ($LASTEXITCODE -ne 0) { throw 'Fixture compilation failed' }
} finally { Pop-Location }
