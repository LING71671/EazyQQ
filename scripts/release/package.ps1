$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$package = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json
$version = $package.version
if ($version -notmatch '^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$') { throw 'Invalid release version' }
$releaseDir = Join-Path $projectRoot "output/release/$version"
$stageDir = Join-Path $releaseDir ('cli-stage-' + [guid]::NewGuid().ToString('N'))
$installer = Join-Path $projectRoot "src-tauri/target/release/bundle/nsis/EazyQQ_${version}_x64-setup.exe"
$cli = Join-Path $projectRoot 'src-tauri/target/release/eazyqq_cli.exe'
foreach ($path in @($installer, $cli)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Release artifact missing: $path. Run pnpm tauri:build first." }
}
Push-Location $projectRoot
try {
    & node scripts/version/check.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Version validation failed' }
    & node scripts/release/check-resources.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Resource validation failed' }
    $binaryVersion = & $cli version --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $binaryVersion.version -ne $version) { throw 'CLI binary version differs from the source manifest' }
    New-Item -ItemType Directory -Path (Join-Path $stageDir 'bin') -Force | Out-Null
    Copy-Item -LiteralPath $cli -Destination (Join-Path $stageDir 'bin/eazyqq_cli.exe')
    Copy-Item -LiteralPath $cli -Destination (Join-Path $stageDir 'bin/ezq.exe')
    Copy-Item -LiteralPath (Join-Path $projectRoot 'src-tauri/resources/napcat') -Destination (Join-Path $stageDir 'napcat') -Recurse
    Copy-Item -LiteralPath (Join-Path $projectRoot 'docs') -Destination (Join-Path $stageDir 'docs') -Recurse
    foreach ($file in @('README.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'AGENTS.md', 'LICENSE', 'llms.txt', 'llms-full.txt')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $file) -Destination $stageDir
    }
    foreach ($source in @($installer, $cli)) { Copy-Item -LiteralPath $source -Destination $releaseDir -Force }
    Copy-Item -LiteralPath $cli -Destination (Join-Path $releaseDir 'ezq.exe') -Force
    $zip = Join-Path $releaseDir 'eazyqq-cli-windows-x64.zip'
    Compress-Archive -Path (Join-Path $stageDir '*') -DestinationPath $zip -Force
    $assets = @(Get-Item -LiteralPath (Join-Path $releaseDir ([IO.Path]::GetFileName($installer))), (Join-Path $releaseDir 'eazyqq_cli.exe'), (Join-Path $releaseDir 'ezq.exe'), $zip)
    $revision = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the release source revision' }
    $manifest = [ordered]@{
        version = $version
        tag = "v$version"
        sourceRevision = $revision
        publishedAt = [DateTime]::UtcNow.ToString('o')
        notesUrl = "https://github.com/LING71671/EazyQQ/releases/tag/v$version"
        assets = @($assets | ForEach-Object {
            [ordered]@{name=$_.Name;size=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant();url="https://github.com/LING71671/EazyQQ/releases/download/v$version/$($_.Name)"}
        })
    }
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $releaseDir 'latest.json') -Encoding utf8
    $hashFiles = @($assets) + @(Get-Item -LiteralPath (Join-Path $releaseDir 'latest.json'))
    $hashFiles | Sort-Object Name | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name } |
        Set-Content -LiteralPath (Join-Path $releaseDir 'SHA256SUMS.txt') -Encoding utf8
    # Remove only this run's verified staging directory; keep the versioned artifacts.
    $resolvedStage = (Resolve-Path -LiteralPath $stageDir).Path
    $resolvedRelease = (Resolve-Path -LiteralPath $releaseDir).Path.TrimEnd('\') + '\'
    if (-not $resolvedStage.StartsWith($resolvedRelease, [StringComparison]::OrdinalIgnoreCase)) { throw 'Staging path escaped the release directory' }
    Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    Get-ChildItem -LiteralPath $releaseDir -File | Select-Object Name,Length
} finally { Pop-Location }
