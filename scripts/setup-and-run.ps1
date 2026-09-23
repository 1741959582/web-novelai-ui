# NAI Studio Web UI - 一键检查/安装开发环境并启动
# 用法：双击 start.bat（启动）  或  start.bat build（打包安装程序）
param([string]$Mode = "dev")

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Set-Location (Split-Path -Parent $PSScriptRoot)

function Info($msg)  { Write-Host "[*] $msg" -ForegroundColor Cyan }
function Ok($msg)    { Write-Host "[OK] $msg" -ForegroundColor Green }
function Warn($msg)  { Write-Host "[!] $msg" -ForegroundColor Yellow }
function Fail($msg)  { Write-Host "[X] $msg" -ForegroundColor Red; exit 1 }

function Refresh-Path {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    $env:Path = "$machine;$user;$cargo"
}

function Has($cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }

function Test-Run($exe, $arg) {
    if (-not (Has $exe)) { return $false }
    try { & $exe $arg *> $null; return ($LASTEXITCODE -eq 0) } catch { return $false }
}

function Winget-Install($id, $extra = @()) {
    if (-not (Has "winget")) { return }
    Info "winget 安装 $id ..."
    $wargs = @("install", "--id", $id, "-e", "--silent", "--accept-source-agreements", "--accept-package-agreements") + $extra
    & winget @wargs
    Refresh-Path
}

function Download($url, $out) {
    Info "下载 $url"
    Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing
}

Refresh-Path
Write-Host ""
Write-Host "==== NAI Studio Web UI 环境检查 ====" -ForegroundColor Magenta
if (-not (Has "winget")) {
    Warn "没有找到 winget。缺少的软件会尝试直接下载安装；Node.js 需要 winget 或手动安装。"
    Warn "可在 Microsoft Store 搜索「应用安装程序」安装 winget。"
}

# ---------------- 1. Node.js ----------------
if (Test-Run "node" "-v") {
    Ok "Node.js $(node -v)"
} else {
    Winget-Install "OpenJS.NodeJS.LTS"
    if (-not (Test-Run "node" "-v")) {
        Fail "Node.js 安装失败。请从 https://nodejs.org 下载 LTS 版安装后，重新双击 start.bat。"
    }
    Ok "Node.js 已安装 $(node -v)"
}

# ---------------- 2. MSVC C++ 编译工具 ----------------
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
function Has-Msvc {
    if (-not (Test-Path $vswhere)) { return $false }
    $p = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$p
}
if (Has-Msvc) {
    Ok "MSVC C++ 编译工具"
} else {
    Warn "缺少 C++ 编译工具（Rust 在 Windows 上需要），开始安装，约需下载 2~4 GB，会弹出管理员确认..."
    $override = "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    Winget-Install "Microsoft.VisualStudio.2022.BuildTools" @("--override", $override)
    if (-not (Has-Msvc)) {
        $exe = Join-Path $env:TEMP "vs_buildtools.exe"
        Download "https://aka.ms/vs/17/release/vs_buildtools.exe" $exe
        Start-Process -FilePath $exe -ArgumentList $override.Split(" ") -Wait -Verb RunAs
    }
    if (-not (Has-Msvc)) {
        Fail "C++ 编译工具安装失败。请手动安装 Visual Studio Build Tools 并勾选「使用 C++ 的桌面开发」。"
    }
    Ok "MSVC C++ 编译工具已安装"
}

# ---------------- 3. Rust ----------------
if (-not (Test-Run "cargo" "-V")) {
    if (Has "rustup") {
        Info "rustup 已存在，安装 stable-msvc 工具链..."
        & rustup default stable-msvc
    } else {
        Winget-Install "Rustlang.Rustup"
        if (Has "rustup") {
            & rustup default stable-msvc
        } else {
            $exe = Join-Path $env:TEMP "rustup-init.exe"
            Download "https://win.rustup.rs/x86_64" $exe
            & $exe -y --default-toolchain stable-msvc --profile minimal
        }
    }
    Refresh-Path
    if (-not (Test-Run "cargo" "-V")) {
        Fail "Rust 安装失败。请从 https://rustup.rs 手动安装后，重新双击 start.bat。"
    }
}
Ok "Rust $(cargo -V)"

# ---------------- 4. WebView2 ----------------
$wv2Keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)
$hasWv2 = $wv2Keys | Where-Object { (Get-ItemProperty $_ -ErrorAction SilentlyContinue).pv } | Select-Object -First 1
if ($hasWv2) {
    Ok "WebView2"
} else {
    Winget-Install "Microsoft.EdgeWebView2Runtime"
    Ok "WebView2（已尝试安装）"
}

# ---------------- 4.5 DirectML.dll（打包资源，缺了会编译失败） ----------------
$resDir = "src-tauri\resources"
$dml = Join-Path $resDir "DirectML.dll"
if (-not (Test-Path $dml)) {
    New-Item -ItemType Directory -Force -Path $resDir | Out-Null
    $found = Get-ChildItem "src-tauri\target" -Recurse -Filter "DirectML.dll" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) {
        Copy-Item $found.FullName $dml
    } else {
        Info "下载 DirectML.dll（微软官方 NuGet 包 Microsoft.AI.DirectML 1.15.4）..."
        $pkg = Join-Path $env:TEMP "directml.zip"
        $out = Join-Path $env:TEMP "directml_pkg"
        Download "https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/1.15.4" $pkg
        if (Test-Path $out) { Remove-Item $out -Recurse -Force }
        Expand-Archive -Path $pkg -DestinationPath $out -Force
        $src = Join-Path $out "bin\x64-win\DirectML.dll"
        if (-not (Test-Path $src)) {
            $src = (Get-ChildItem $out -Recurse -Filter "DirectML.dll" | Where-Object { $_.FullName -match "x64" } | Select-Object -First 1).FullName
        }
        if (-not $src) { Fail "没能取得 DirectML.dll，请检查网络/代理后重试。" }
        Copy-Item $src $dml
    }
}
Ok "DirectML.dll"

# ---------------- 5. npm 依赖 ----------------
if (-not (Test-Path "node_modules\@tauri-apps\cli")) {
    Info "安装 npm 依赖..."
    & npm.cmd install
    if ($LASTEXITCODE -ne 0) { Fail "npm install 失败，请检查网络或代理。" }
}
Ok "npm 依赖"

# ---------------- 6. 启动 / 打包 ----------------
Write-Host ""
if ($Mode -eq "build") {
    Info "打包安装程序（首次编译需要较长时间）..."
    & npm.cmd run build
    if ($LASTEXITCODE -ne 0) { Fail "打包失败，请把上面的 error 信息复制出来排查。" }
    Ok "完成，安装包在 src-tauri\target\release\bundle\"
    Start-Process (Resolve-Path "src-tauri\target\release\bundle")
} else {
    Info "启动 Tauri 桌面端（首次需要编译 Rust，可能要 5~15 分钟）..."
    & npm.cmd run dev
    if ($LASTEXITCODE -ne 0) { Fail "启动失败，请把上面的 error 信息复制出来排查。" }
}
