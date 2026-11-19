@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall-OmniScreenVeil.ps1"
set "uninstallExit=%ERRORLEVEL%"
echo.
pause
exit /b %uninstallExit%
