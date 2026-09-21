@echo off
chcp 65001 >nul
cd /d "%~dp0"
if not exist node_modules (
  echo 正在安装依赖...
  call npm install
)
echo 启动 Tauri 桌面端...
call npm run dev
if errorlevel 1 pause
