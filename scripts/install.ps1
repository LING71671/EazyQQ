# EazyQQ CLI 一键安装脚本
# 用法: irm https://raw.githubusercontent.com/LING71671/EazyQQ/main/scripts/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "LING71671/EazyQQ"
$InstallDir = "$env:USERPROFILE\.eazyqq"
$BinDir = "$InstallDir\bin"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "       EazyQQ CLI 一键安装程序            " -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# 1. 确保安装目录存在
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
}

# 2. 查询最新版本
Write-Host "==> 正在查询最新 Release 版本..." -ForegroundColor Yellow
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
$Release = $null
try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers @{ 'User-Agent' = 'EazyQQ-Installer' }
    $TagName = $Release.tag_name
} catch {
    Write-Warning "无法访问 GitHub Releases API，尝试默认标签..."
    $TagName = "v0.3.0-beta"
}

Write-Host "==> 目标版本: $TagName" -ForegroundColor Green

# 3. 寻找 CLI zip 资源并下载
$Asset = $null
if ($Release -and $Release.assets) {
    $Asset = $Release.assets | Where-Object { $_.name -like '*cli*windows*.zip' -or $_.name -like '*eazyqq-cli*.zip' } | Select-Object -First 1
}

$ZipUrl = if ($Asset) { $Asset.browser_download_url } else { "https://github.com/$Repo/releases/download/$TagName/eazyqq-cli-windows-x64.zip" }
$TempZip = "$env:TEMP\eazyqq-cli-$TagName.zip"

Write-Host "==> 正在下载 EazyQQ CLI ($ZipUrl)..." -ForegroundColor Yellow
$Downloaded = $false
try {
    Invoke-WebRequest -Uri $ZipUrl -OutFile $TempZip
    Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force
    Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
    $Downloaded = $true
} catch {
    Write-Warning "下载便携 ZIP 失败，尝试单文件备用源: $_"
}

if (-not $Downloaded) {
    $ExeUrl = "https://github.com/$Repo/releases/download/$TagName/eazyqq_cli.exe"
    Write-Host "==> 正在下载单文件 eazyqq_cli.exe ($ExeUrl)..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $ExeUrl -OutFile "$BinDir\eazyqq_cli.exe"
}

# 4. 配置用户 PATH 环境变量
$UserPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($UserPath -notlike "*$BinDir*") {
    Write-Host "==> 正在将 $BinDir 添加到系统 PATH 环境变量..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$BinDir;$UserPath", [EnvironmentVariableTarget]::User)
}
$env:Path = "$BinDir;$env:Path"

# 5. 验证执行与提示
$ExePath = "$BinDir\eazyqq_cli.exe"
if (Test-Path $ExePath) {
    Write-Host ""
    Write-Host "✓ EazyQQ CLI 安装成功！" -ForegroundColor Green
    & $ExePath version
    Write-Host ""
    Write-Host "常用指令速查:" -ForegroundColor Cyan
    Write-Host "  eazyqq_cli qr                 # 终端 ANSI 二维码扫码登录" -ForegroundColor White
    Write-Host "  eazyqq_cli qr --browser       # 自动弹起浏览器扫码登录" -ForegroundColor White
    Write-Host "  eazyqq_cli status             # 查看协议与登录状态" -ForegroundColor White
    Write-Host "  eazyqq_cli contacts           # 查看好友与群聊列表" -ForegroundColor White
    Write-Host "  eazyqq_cli instances list     # 管理多开账号实例池" -ForegroundColor White
    Write-Host "  eazyqq_cli mcp                # 启动 stdio MCP 服务接入外部 AI" -ForegroundColor White
    Write-Host ""
} else {
    Write-Error "安装异常：未能在 $BinDir 找到 eazyqq_cli.exe，请检查网络权限。"
}
