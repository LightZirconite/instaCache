# instaCache installer for Windows.
#
#   powershell -ExecutionPolicy Bypass -File install.ps1     (or double-click install.cmd)
#
# Installs the unpacked archive this script sits in for the current user,
# into %LOCALAPPDATA%\Programs\instaCache by default: no administrator rights,
# nothing written outside the user's own profile. Adds instaCache to the Start
# menu, to Settings > Apps for uninstalling, and as the handler of
# instacache: links.
#
# `instacache --update` runs the install.ps1 of the *newer* archive, with
# -Yes and -Prefix, so those two options are a stable interface: renaming
# either breaks every existing install's updates. See "The installer's option
# contract" in AGENTS.md.
#
# This file is ASCII on purpose: Windows PowerShell 5.1 reads a script without
# a byte order mark in the legacy code page, and anything else arrives mangled.

param(
    # Where to install.
    [string]$Prefix = '',
    # Never ask, and do not start instaCache afterwards. What an update uses.
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2

$App = 'instaCache'
$Exe = 'instacache.exe'

function Step([string]$Text) { Write-Host "==> $Text" -ForegroundColor Cyan }
function Ok([string]$Text) { Write-Host "  ok $Text" -ForegroundColor Green }
function Warn([string]$Text) { Write-Host "warn $Text" -ForegroundColor Yellow }

if (-not $Prefix) {
    $Prefix = Join-Path $env:LOCALAPPDATA 'Programs\instaCache'
}

$source = (Resolve-Path -LiteralPath $PSScriptRoot).Path.TrimEnd('\')
if (-not (Test-Path -LiteralPath (Join-Path $source $Exe))) {
    throw "$Exe is not beside this script. Run install.ps1 from inside the unpacked archive."
}
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
$prefix = (Resolve-Path -LiteralPath $Prefix).Path.TrimEnd('\')
if ($source -ieq $prefix) {
    throw "This is the installed copy already. Run the install.ps1 of a newer archive instead."
}

$version = ''
$versionFile = Join-Path $source 'version.txt'
if (Test-Path -LiteralPath $versionFile) {
    $version = (Get-Content -LiteralPath $versionFile -Raw).Trim()
}

Write-Host ''
Write-Host "$App installer $version" -ForegroundColor White
Write-Host ''

# ------------------------------------------------------------------ files ---

Step "Installing into $prefix"

# Files that a running copy was still holding at the last update, renamed
# aside then. Free now, unless that copy is still running.
Get-ChildItem -LiteralPath $prefix -Recurse -File -Filter '*.instacache-old' -ErrorAction SilentlyContinue |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue }

$copied = 0
$movedAside = 0
foreach ($file in Get-ChildItem -LiteralPath $source -Recurse -File) {
    $relative = $file.FullName.Substring($source.Length + 1)
    if ($relative -ieq 'get.ps1') {
        # The one-command installer is the updater once installed: it fetches
        # the newest archive and runs its install.ps1.
        $relative = 'update.ps1'
    }
    $target = Join-Path $prefix $relative
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
    try {
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    } catch {
        # Windows will not overwrite a running program or a DLL it has loaded,
        # but lets either be renamed. The running copy keeps the renamed file
        # and the next start loads the new one: the same thing replacing a
        # binary under a running copy does on Linux.
        $aside = (Split-Path -Leaf $target) + '.' + [guid]::NewGuid().ToString('N') + '.instacache-old'
        Rename-Item -LiteralPath $target -NewName $aside -Force
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
        $movedAside++
    }
    $copied++
}
# A download carries the mark of the web, and SmartScreen would stop the first
# start of every file that has it. These are the files that were just checked.
Get-ChildItem -LiteralPath $prefix -Recurse -File | Unblock-File -ErrorAction SilentlyContinue
Ok "$copied files"
if ($movedAside -gt 0) {
    Ok "$movedAside in use by the running copy; the new ones take over when it restarts"
}

$exePath = Join-Path $prefix $Exe

# --------------------------------------------------------------- shortcut ---

Step 'Adding it to the Start menu'
$programs = [Environment]::GetFolderPath('Programs')
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut((Join-Path $programs "$App.lnk"))
$link.TargetPath = $exePath
$link.WorkingDirectory = $prefix
$link.IconLocation = "$exePath,0"
$link.Description = 'Instagram as a desktop application, with a cache that persists'
$link.Save()
Ok (Join-Path $programs "$App.lnk")

# ---------------------------------------------------------------- registry ---

Step 'Registering instacache: links and the uninstaller'

# instacache:https://www.instagram.com/... from a browser, which is what
# extras/open-in-instacache.user.js sends.
$protocol = 'HKCU:\Software\Classes\instacache'
New-Item -Path "$protocol\shell\open\command" -Force | Out-Null
New-Item -Path "$protocol\DefaultIcon" -Force | Out-Null
Set-ItemProperty -Path $protocol -Name '(default)' -Value 'URL:instaCache'
Set-ItemProperty -Path $protocol -Name 'URL Protocol' -Value ''
Set-ItemProperty -Path "$protocol\DefaultIcon" -Name '(default)' -Value "`"$exePath`",0"
Set-ItemProperty -Path "$protocol\shell\open\command" -Name '(default)' -Value "`"$exePath`" `"%1`""

# Settings > Apps, per user.
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\instaCache'
$uninstaller = Join-Path $prefix 'uninstall.ps1'
New-Item -Path $uninstallKey -Force | Out-Null
Set-ItemProperty -Path $uninstallKey -Name 'DisplayName' -Value $App
Set-ItemProperty -Path $uninstallKey -Name 'DisplayVersion' -Value $version
Set-ItemProperty -Path $uninstallKey -Name 'Publisher' -Value 'LightZirconite'
Set-ItemProperty -Path $uninstallKey -Name 'DisplayIcon' -Value "$exePath,0"
Set-ItemProperty -Path $uninstallKey -Name 'InstallLocation' -Value $prefix
Set-ItemProperty -Path $uninstallKey -Name 'URLInfoAbout' -Value 'https://github.com/LightZirconite/instaCache'
Set-ItemProperty -Path $uninstallKey -Name 'UninstallString' -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstaller`""
Set-ItemProperty -Path $uninstallKey -Name 'NoModify' -Value 1 -Type DWord
Set-ItemProperty -Path $uninstallKey -Name 'NoRepair' -Value 1 -Type DWord
$size = (Get-ChildItem -LiteralPath $prefix -Recurse -File | Measure-Object -Property Length -Sum).Sum
Set-ItemProperty -Path $uninstallKey -Name 'EstimatedSize' -Value ([int]($size / 1KB)) -Type DWord
Ok 'done'

# ------------------------------------------------------------------ sites ---

# The sites instaCache ships with, and every site's shortcut brought up to
# date, exactly as install.sh does on Linux.
Step 'Setting up the extra sites'
$setup = Start-Process -FilePath $exePath -ArgumentList '--setup-sites' -Wait -PassThru -WindowStyle Hidden
if ($setup.ExitCode -eq 0) {
    Ok 'done'
} else {
    Warn "instacache --setup-sites exited with $($setup.ExitCode); the sites can be added later"
}

Write-Host ''
Write-Host "$App is installed. Open it from the Start menu." -ForegroundColor Green
Write-Host "Uninstall from Settings > Apps, or with $uninstaller"
Write-Host ''

if (-not $Yes) {
    Start-Process -FilePath $exePath
}
