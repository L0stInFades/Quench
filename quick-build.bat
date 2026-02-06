@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo.
echo ========================================
echo    ZipX 快速构建脚本
echo ========================================
echo.

REM 检查当前目录
if not exist "ui" (
    echo [错误] 未找到 ui 目录
    echo 请从项目根目录运行此脚本
    pause
    exit /b 1
)

cd ui || exit /b 1

REM 检查npm
where npm >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [错误] npm 未安装
    echo 请先安装 Node.js: https://nodejs.org/
    pause
    exit /b 1
)

echo [1/4] 检查依赖...
if not exist "node_modules" (
    echo [2/4] 安装 npm 依赖...
    call npm install
    if %ERRORLEVEL% NEQ 0 (
        echo [错误] npm 安装失败
        pause
        exit /b 1
    )
) else (
    echo [√] 依赖已安装
)

echo.
echo [3/4] 构建前端...
call npm run build
if %ERRORLEVEL% NEQ 0 (
    echo [错误] 前端构建失败
    pause
    exit /b 1
)

echo.
echo [4/4] 构建Tauri应用程序...
echo 这是最耗时的步骤，请耐心等待5-10分钟...
echo.

call npx tauri build
if %ERRORLEVEL% EQU 0 (
    echo.
    echo ========================================
    echo    构建完成！
    echo ========================================
    echo.
    echo [位置] 可执行文件:
    echo   ui\src-tauri\target\release\zipx-ui.exe
    echo.
    echo [位置] 安装包:
    dir /s /b src-tauri\target\release\bundle 2>nul
    echo.
    echo [✓] 可以直接运行 zipx-ui.exe
    echo.
) else (
    echo.
    echo [错误] 构建失败，请查看上方错误信息
    echo.
    echo 常见解决方案:
    echo   1. 删除 ui\src-tauri\Cargo.lock 文件
    echo   2. 删除 ui\node_modules 文件夹
    echo   3. 重新运行此脚本
    echo.
    pause
    exit /b 1
)

echo.
echo 按任意键退出...
pause >nul
