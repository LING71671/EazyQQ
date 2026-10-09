function Resolve-CargoTargetDirectory {
    param([Parameter(Mandatory = $true)][string]$ProjectRoot)
    $manifestRoot = Join-Path $ProjectRoot 'src-tauri'
    if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        return Join-Path $manifestRoot 'target'
    }
    if ([IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        return [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }
    return [IO.Path]::GetFullPath((Join-Path $manifestRoot $env:CARGO_TARGET_DIR))
}
