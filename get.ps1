# instaCache for Windows, in one command, from PowerShell:
#
#   irm https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.ps1 | iex
#
# Downloads the newest release archive, verifies it against its published
# checksum, unpacks it and runs the install.ps1 inside it, which installs for
# the current user without administrator rights.
#
# install.ps1 copies this file beside instacache.exe as update.ps1, and that
# is what an update runs, with -Yes and -Prefix, both handed on to the newer
# install.ps1 unchanged.
#
# ASCII only, like install.ps1, for Windows PowerShell 5.1.

param(
    [string]$Prefix = '',
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2
# Windows PowerShell 5.1 still offers TLS 1.0 first, which GitHub refuses, and
# draws a progress bar that makes a large download several times slower.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'

$Repo = if ($env:INSTACACHE_REPO) { $env:INSTACACHE_REPO } else { 'LightZirconite/instaCache' }

function Step([string]$Text) { Write-Host "==> $Text" -ForegroundColor Cyan }
function Ok([string]$Text) { Write-Host "  ok $Text" -ForegroundColor Green }

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'instaCache needs a 64-bit Windows.'
}

Write-Host ''
Write-Host 'instaCache installer' -ForegroundColor White
Write-Host ''

Step 'Looking up the latest release'
$tag = $env:INSTACACHE_VERSION
if (-not $tag) {
    $release = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $tag = $release.tag_name
}
if (-not $tag) {
    throw "Could not reach the GitHub release API. Download the archive by hand from https://github.com/$Repo/releases"
}
$number = $tag.TrimStart('v')
$name = "instacache-$number-windows-x86_64"
$base = "https://github.com/$Repo/releases/download/$tag"
Ok $tag

$work = Join-Path ([IO.Path]::GetTempPath()) ('instacache-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $work | Out-Null
try {
    Step 'Downloading'
    $archive = Join-Path $work "$name.zip"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$name.zip" -OutFile $archive
    Ok "$name.zip"

    Step 'Verifying the download'
    $sums = Join-Path $work "$name.zip.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$name.zip.sha256" -OutFile $sums
    $expected = ((Get-Content -LiteralPath $sums -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Checksum mismatch, refusing to install. Expected $expected, got $actual."
    }
    Ok 'checksum matches'

    Step 'Unpacking'
    Expand-Archive -LiteralPath $archive -DestinationPath $work -Force
    Ok 'done'

    # In a PowerShell of its own, allowed to run this one script: Windows'
    # default execution policy refuses every script file, install.ps1
    # included, even though it lets `irm | iex` run this text. That is the
    # failure this replaced.
    $installer = Join-Path (Join-Path $work $name) 'install.ps1'
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $installer)
    if ($Prefix) { $arguments += @('-Prefix', $Prefix) }
    if ($Yes) { $arguments += '-Yes' }
    & powershell.exe @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "install.ps1 failed with exit code $LASTEXITCODE."
    }
} finally {
    Set-Location -LiteralPath ([IO.Path]::GetTempPath())
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
