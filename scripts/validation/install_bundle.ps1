$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Installer QA runs only on hosted CI, preserving the user installation registry' }
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$installer = Join-Path $projectRoot "src-tauri/target/release/bundle/nsis/EazyQQ_${version}_x64-setup.exe"
$target = Join-Path $projectRoot ('.test-runtime/installed QA-' + [guid]::NewGuid().ToString('N'))
$process = Start-Process -FilePath $installer -ArgumentList @('/S', ('/D=' + $target)) -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer exited with code $($process.ExitCode)" }
foreach ($file in @('eazyqq.exe','eazyqq_cli.exe','resources/napcat/NapCatWinBootMain.exe','resources/napcat/NapCatWinBootHook.dll','resources/napcat/napcat.mjs')) {
    if (-not (Test-Path -LiteralPath (Join-Path $target $file))) { throw "Installed bundle is missing $file" }
}
$cli = Join-Path $target 'eazyqq_cli.exe'
$actual = & $cli version --json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $actual.version -ne $version) { throw 'Installed CLI version does not match the bundle' }
"EAZYQQ_TEST_GUI=$(Join-Path $target 'eazyqq.exe')" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
"EAZYQQ_TEST_CLI=$cli" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
[ordered]@{installed=$true;version=$actual.version;target=$target} | ConvertTo-Json
