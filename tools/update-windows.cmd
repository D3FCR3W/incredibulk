@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0update-windows.ps1" -PackageDirectory "%~dp0."
if errorlevel 1 (
  echo.
  echo Rien ne sera ferme de force. Les sauvegardes sont conservees.
  pause
  exit /b 1
)
timeout /t 3 /nobreak >nul
