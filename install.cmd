@echo off
rem Double-click to install instaCache for this user. Windows will not run a
rem PowerShell script from a double-click, so this runs install.ps1 for it.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
if errorlevel 1 pause
