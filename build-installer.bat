@echo off
REM ZipX Windows Installer Build Script

echo ========================================
echo   Building ZipX Windows Installer
echo ========================================
echo.

cd ui || exit /b 1

REM Check if npm is installed
where npm >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] npm not found. Please install Node.js first.
    echo Download from: https://nodejs.org/
    exit /b 1
)

echo [INFO] npm found
npm --version
echo.

REM Install dependencies if needed
if not exist "node_modules" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if %ERRORLEVEL% NEQ 0 (
        echo [ERROR] Failed to install dependencies
        exit /b 1
    )
)

echo.
echo [INFO] Building Tauri application...
echo This may take 5-10 minutes, please be patient...
echo.

call npm run tauri build
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Build failed
    exit /b 1
)

echo.
echo ========================================
echo   Build Complete!
echo ========================================
echo.
echo [INFO] Executable: ui\src-tauri\target\release\zipx-ui.exe
echo.
echo [INFO] Installer bundles:
dir /s /b src-tauri\target\release\bundle\*.exe 2>nul
dir /s /b src-tauri\target\release\bundle\*.msi 2>nul
echo.

echo [SUCCESS] ZipX Windows installer has been built!
echo.
pause
