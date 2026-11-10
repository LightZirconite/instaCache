# instaCache uninstaller for Windows.
#
#   uninstall.ps1            remove the application, keep your session and settings
#   uninstall.ps1 -Purge     also delete the session, cache and configuration
#
# Settings > Apps runs it without options. It lives in the install directory
# and removes what install.ps1 and instaCache itself added: the files, the
# Start menu shortcuts (instaCache's and every site's), the instacache: link
# handler, the start-at-login entries and the uninstaller's own entry.
#
# ASCII only, like install.ps1, for Windows PowerShell 5.1.

param(
    # Also delete the session, cache and configuration. Signs you out.
    [switch]$Purge,
    # Do not ask before purging.
    [switch]$Yes,
    # The install to remove: the directory this script is in.
    [string]$Prefix = $PSScriptRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2

$App = 'instaCache'

function Step([string]$Text) { Write-Host "==> $Text" -ForegroundColor Cyan }
function Ok([string]$Text) { Write-Host "  ok $Text" -ForegroundColor Green }

$prefix = (Resolve-Path -LiteralPath $Prefix).Path.TrimEnd('\')
if (-not (Test-Path -LiteralPath (Join-Path $prefix 'instacache.exe'))) {
    throw "$prefix does not hold an instaCache install."
}
$config = Join-Path $env:APPDATA 'instacache'
$local = Join-Path $env:LOCALAPPDATA 'instacache'

Step 'Closing instaCache'
Get-Process -Name 'instacache' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) } |
    Stop-Process -Force
Get-Process -Name 'QtWebEngineProcess' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) } |
    Stop-Process -Force
Ok 'done'

Step 'Removing the Start menu shortcuts'
$programs = [Environment]::GetFolderPath('Programs')
Remove-Item -LiteralPath (Join-Path $programs "$App.lnk") -Force -ErrorAction SilentlyContinue
# Every site keeps the path of its shortcut in its profile; see sites.rs.
$profiles = Join-Path $config 'profiles'
if (Test-Path -LiteralPath $profiles) {
    foreach ($record in Get-ChildItem -LiteralPath $profiles -Recurse -File -Filter 'start-menu-entry') {
        $link = (Get-Content -LiteralPath $record.FullName -Raw).Trim()
        if ($link) {
            Remove-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $record.FullName -Force -ErrorAction SilentlyContinue
    }
}
Ok 'done'

Step 'Removing the registry entries'
Remove-Item -Path 'HKCU:\Software\Classes\instacache' -Recurse -Force -ErrorAction SilentlyContinue
foreach ($key in @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run')) {
    if (Test-Path -Path $key) {
        $values = (Get-Item -Path $key).GetValueNames() |
            Where-Object { $_ -eq $App -or $_.StartsWith("$App (") }
        foreach ($value in $values) {
            Remove-ItemProperty -Path $key -Name $value -Force -ErrorAction SilentlyContinue
        }
    }
}
Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\instaCache' -Recurse -Force -ErrorAction SilentlyContinue
Ok 'done'

if ($Purge) {
    $sure = $Yes
    if (-not $sure) {
        $answer = Read-Host "Delete your session, cache and settings in $config and $local? This signs you out. [y/N]"
        $sure = $answer -match '^(y|yes|o|oui)$'
    }
    if ($sure) {
        Step 'Deleting the session, cache and settings'
        Remove-Item -LiteralPath $config -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $local -Recurse -Force -ErrorAction SilentlyContinue
        Ok 'done'
    }
}

Step "Removing $prefix"
# Not from inside it: a directory that is the current one cannot be deleted.
Set-Location -LiteralPath $env:TEMP
Remove-Item -LiteralPath $prefix -Recurse -Force
Ok 'done'

Write-Host ''
Write-Host "$App is uninstalled." -ForegroundColor Green
if (-not $Purge) {
    Write-Host "Your session and settings are kept in $config and $local."
    Write-Host 'Run uninstall.ps1 -Purge from a new archive to delete them too.'
}
