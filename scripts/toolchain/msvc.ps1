function Enable-MsvcEnvironment {
    $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswherePath)) { return }
    $installation = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installation) { return }
    $vcvarsPath = Join-Path $installation 'VC/Auxiliary/Build/vcvars64.bat'
    foreach ($environmentLine in (& cmd.exe /d /s /c "call `"$vcvarsPath`" >nul && set")) {
        if ($environmentLine -match '^([^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
        }
    }
}
