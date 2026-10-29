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
    # vcvars64.bat resolves the Windows SDK through the registry. When reg.exe is
    # unavailable (locked-down hosts, hardened agents) the SDK directories are
    # silently dropped and cl.exe fails on stdio.h. Fall back to the newest
    # on-disk kit so the toolchain still completes without touching the registry.
    if ($env:INCLUDE -notmatch 'ucrt') {
        $kitsInclude = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/Include'
        if (Test-Path -LiteralPath $kitsInclude) {
            $sdkVersion = (Get-ChildItem -LiteralPath $kitsInclude -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
            $kitsLib = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/Lib'
            $env:INCLUDE = "$env:INCLUDE;$kitsInclude\$sdkVersion\ucrt;$kitsInclude\$sdkVersion\um;$kitsInclude\$sdkVersion\shared"
            $env:LIB = "$env:LIB;$kitsLib\$sdkVersion\ucrt\x64;$kitsLib\$sdkVersion\um\x64"
        }
    }
}
