$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    # The installer ships a real CLI alongside the GUI, sharing the same data root.
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'cargo.ps1') build --release --bin eazyqq_cli
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally { Pop-Location }
