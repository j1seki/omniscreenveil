@echo off
setlocal DisableDelayedExpansion
set "uninstallScript=%~dp0Uninstall-OmniScreenVeil.ps1"
cd /d "%TEMP%"
setlocal EnableDelayedExpansion
rem Read this entire block before PowerShell removes the launcher.
(
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "!uninstallScript!"
    set "uninstallExit=!ERRORLEVEL!"
    echo.
    pause
    exit /b !uninstallExit!
)
