# EazyQQ CLI 全自动一键安装脚本（含全量依赖与环境就绪体检）
# 用法: irm https://raw.githubusercontent.com/LING71671/EazyQQ/main/scripts/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "LING71671/EazyQQ"
$InstallDir = "$env:USERPROFILE\.eazyqq"
$BinDir = "$InstallDir\bin"
$NapCatDir = "$InstallDir\napcat"
$ConfigDir = "$NapCatDir\config"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "    EazyQQ CLI 全自动安装与环境初始化    " -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# 1. 确保核心目录就绪
foreach ($d in @($BinDir, $ConfigDir)) {
    if (-not (Test-Path $d)) {
        New-Item -ItemType Directory -Path $d -Force | Out-Null
    }
}

# 2. 查询最新版本
Write-Host "==> [1/5] 查询最新 Release 版本..." -ForegroundColor Yellow
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
$Release = $null
try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers @{ 'User-Agent' = 'EazyQQ-Installer' }
    $TagName = $Release.tag_name
} catch {
    throw "Cannot resolve the current release: $($_.Exception.Message). Retry after restoring network access."
}
Write-Host "    目标版本: $TagName" -ForegroundColor Green

# 3. 下载并解压 CLI 及预置运行时
Write-Host "==> [2/5] 获取 EazyQQ CLI 核心套件..." -ForegroundColor Yellow
$Asset = $null
if ($Release -and $Release.assets) {
    $Asset = $Release.assets | Where-Object { $_.name -like '*cli*windows*.zip' -or $_.name -like '*eazyqq-cli*.zip' } | Select-Object -First 1
}

$ZipUrl = if ($Asset) { $Asset.browser_download_url } else { "https://github.com/$Repo/releases/download/$TagName/eazyqq-cli-windows-x64.zip" }
$TempZip = "$env:TEMP\eazyqq-cli-$TagName.zip"
$Downloaded = $false

try {
    Invoke-WebRequest -Uri $ZipUrl -OutFile $TempZip
    Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force
    Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
    $Downloaded = $true
} catch {
    Write-Warning "便携 ZIP 包暂未生成或下载受阻，启动容灾流水线..."
}

if (-not (Test-Path "$BinDir\eazyqq_cli.exe")) {
    $ExeUrl = "https://github.com/$Repo/releases/download/$TagName/eazyqq_cli.exe"
    Write-Host "    下载独立可执行文件 $ExeUrl ..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $ExeUrl -OutFile "$BinDir\eazyqq_cli.exe"
}

# 4. 确保 NapCat 依赖核心完整
Write-Host "==> [3/5] 校验 NapCat 协议运行时依赖..." -ForegroundColor Yellow
$NapCatCoreFiles = @("NapCatWinBootMain.exe", "NapCatWinBootHook.dll", "napcat.mjs")
$NeedsNapCat = $false
foreach ($f in $NapCatCoreFiles) {
    if (-not (Test-Path "$NapCatDir\$f")) {
        $NeedsNapCat = $true
        break
    }
}

if ($NeedsNapCat) {
    Write-Host "    正在从官方镜像自动补齐 NapCat 协议组件..." -ForegroundColor Yellow
    try {
        $napcatRelease = Invoke-RestMethod -Uri 'https://api.github.com/repos/NapNeko/NapCatQQ/releases/latest' -Headers @{ 'User-Agent' = 'EazyQQ-Installer' }
        $shellAsset = $napcatRelease.assets | Where-Object { $_.name -eq 'NapCat.Shell.zip' } | Select-Object -First 1
        if ($shellAsset) {
            $shellZip = "$env:TEMP\NapCat.Shell.zip"
            Invoke-WebRequest -Uri $shellAsset.browser_download_url -OutFile $shellZip
            Expand-Archive -Path $shellZip -DestinationPath $NapCatDir -Force
            Remove-Item -Path $shellZip -Force -ErrorAction SilentlyContinue
            Write-Host "    ✓ NapCat Shell 补齐完毕" -ForegroundColor Green
        }
    } catch {
        Write-Warning "NapCat 在线拉取受限，如运行异常可手动将 napcat 目录置于 $InstallDir\napcat"
    }
} else {
    Write-Host "    ✓ NapCat 核心组件齐全" -ForegroundColor Green
}

# 5. 自动探测宿主机 QQNT 路径
Write-Host "==> [4/5] 自动探测宿主机官方 QQNT..." -ForegroundColor Yellow
$DetectedQQ = $null

# 5.1 探测正在运行的 QQ
$proc = Get-Process -Name QQ -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Path -First 1
if ($proc -and (Test-Path $proc)) {
    $DetectedQQ = $proc
}

# 5.2 探测注册表
if (-not $DetectedQQ) {
    $regPaths = @(
        'HKLM:\SOFTWARE\WOW6432Node\Tencent\QQNT',
        'HKCU:\Software\Tencent\QQNT'
    )
    foreach ($rp in $regPaths) {
        $inst = (Get-ItemProperty -Path $rp -ErrorAction SilentlyContinue).Install
        if ($inst) {
            $candidate = Join-Path $inst "QQ.exe"
            if (Test-Path $candidate) {
                $DetectedQQ = $candidate
                break
            }
        }
    }
}

# 5.3 探测常见默认安装路径
if (-not $DetectedQQ) {
    $commonPaths = @(
        "C:\Program Files\Tencent\QQNT\QQ.exe",
        "C:\Program Files (x86)\Tencent\QQNT\QQ.exe",
        "D:\Program Files\Tencent\QQNT\QQ.exe",
        "E:\Program Files\Tencent\QQNT\QQ.exe"
    )
    foreach ($cp in $commonPaths) {
        if (Test-Path $cp) {
            $DetectedQQ = $cp
            break
        }
    }
}

$QQPathTxt = "$ConfigDir\qq_path.txt"
if ($DetectedQQ) {
    Set-Content -Path $QQPathTxt -Value $DetectedQQ -Encoding UTF8
    Write-Host "    ✓ 自动绑定 QQ 路径: $DetectedQQ" -ForegroundColor Green
} else {
    Write-Warning "    未在系统注册表或默认路径中检测到 QQ.exe。"
    Write-Host "    提示：如果你尚未安装最新官方 QQNT，请前往 https://im.qq.com/pcqq 下载安装；" -ForegroundColor White
    Write-Host "    或将 QQ.exe 路径写入: $QQPathTxt" -ForegroundColor White
}

# 确保基础配置文件存在
if (-not (Test-Path "$ConfigDir\webui.json")) {
    $tokenBytes = New-Object byte[] 6
    (New-Object System.Security.Cryptography.RNGCryptoServiceProvider).GetBytes($tokenBytes)
    $tokenHex = ($tokenBytes | ForEach-Object { $_.ToString("x2") }) -join ""
    $webuiJson = @{
        host = "0.0.0.0"
        port = 6099
        token = $tokenHex
        loginRate = 10
        autoLoginAccount = ""
    } | ConvertTo-Json
    Set-Content -Path "$ConfigDir\webui.json" -Value $webuiJson -Encoding UTF8
}

if (-not (Test-Path "$ConfigDir\napcat.json")) {
    $napcatJson = @{
        fileLog = $true
        consoleLog = $true
        fileLogLevel = "debug"
        consoleLogLevel = "info"
        packetBackend = "auto"
        packetServer = ""
        o3HookMode = 1
    } | ConvertTo-Json
    Set-Content -Path "$ConfigDir\napcat.json" -Value $napcatJson -Encoding UTF8
}

# 6. 配置用户全局 PATH 环境变量
Write-Host "==> [5/5] 注册全局 PATH 环境变量..." -ForegroundColor Yellow
$UserPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($UserPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$BinDir;$UserPath", [EnvironmentVariableTarget]::User)
    Write-Host "    ✓ 已将 $BinDir 加入系统 PATH" -ForegroundColor Green
} else {
    Write-Host "    ✓ PATH 中已包含 $BinDir" -ForegroundColor Green
}
$env:Path = "$BinDir;$env:Path"

# 确保 ezq.exe 与 eazyqq_cli.exe 双向存在
if (Test-Path "$BinDir\eazyqq_cli.exe" -and -not (Test-Path "$BinDir\ezq.exe")) {
    Copy-Item "$BinDir\eazyqq_cli.exe" "$BinDir\ezq.exe" -Force
} elseif (Test-Path "$BinDir\ezq.exe" -and -not (Test-Path "$BinDir\eazyqq_cli.exe")) {
    Copy-Item "$BinDir\ezq.exe" "$BinDir\eazyqq_cli.exe" -Force
}

# 7. 完成提示与指令指南
$ExePath = if (Test-Path "$BinDir\ezq.exe") { "$BinDir\ezq.exe" } else { "$BinDir\eazyqq_cli.exe" }
if (Test-Path $ExePath) {
    Write-Host ""
    Write-Host "==========================================" -ForegroundColor Green
    Write-Host "       EazyQQ 全环境安装成功！            " -ForegroundColor Green
    Write-Host "==========================================" -ForegroundColor Green
    Write-Host ""
    & $ExePath version
    Write-Host ""
    Write-Host "核心无头常用指令 (短命令: ezq):" -ForegroundColor Cyan
    Write-Host "  ezq qr                 # 终端字符扫码登录（手机 QQ 扫一扫即登）" -ForegroundColor White
    Write-Host "  ezq qr --browser       # 浏览器弹出扫码" -ForegroundColor White
    Write-Host "  ezq status             # 查看协议在线态" -ForegroundColor White
    Write-Host "  ezq send --target <ID> --text <内容> # 真实收发消息" -ForegroundColor White
    Write-Host "  ezq instances list     # 查看/管理多开账号实例池" -ForegroundColor White
    Write-Host "  ezq mcp                # 启动 stdio MCP 服务供 Cursor/Claude 接入" -ForegroundColor White
    Write-Host ""
} else {
    Write-Error "安装异常：未能在 $BinDir 找到 ezq.exe 或 eazyqq_cli.exe。"
}
