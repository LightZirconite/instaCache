# Assembles the Windows release archive from a release build.
#
#   scripts/package-windows.ps1 -Version 2.14.0
#
# Run from the repository root after `cargo build --release`, with the Qt it
# was built against on PATH (windeployqt comes from there). Writes
# dist/instacache-<version>-windows-x86_64/, its .zip and the .zip.sha256 in
# the format sha256sum writes, which is what get.ps1 and the release notes
# read. Both CI and the Release workflow run this, so what CI checks is what
# ships.
#
# Windows has no system Qt to link against, so unlike the Linux archive this
# one carries Qt's own DLLs and Qt WebEngine beside instacache.exe, still
# linked dynamically. See "Non-negotiables" in AGENTS.md.

param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2

$name = "instacache-$Version-windows-x86_64"
$dist = Join-Path (Get-Location) 'dist'
$out = Join-Path $dist $name
$exe = 'target\release\instacache.exe'

if (-not (Test-Path -LiteralPath $exe)) {
    throw "$exe does not exist; build with cargo build --release first."
}
Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $out | Out-Null

Copy-Item -LiteralPath $exe -Destination $out

# The QML modules the scene imports. The tray icon's Qt.labs.platform is
# imported from a string at run time (see main.qml), where windeployqt's
# scanner cannot see it, so it is named in a file of its own.
$imports = Join-Path $dist 'imports'
New-Item -ItemType Directory -Force -Path $imports | Out-Null
Set-Content -LiteralPath (Join-Path $imports 'runtime.qml') -Encoding ascii -Value @'
import QtQuick
import Qt.labs.platform
Item {}
'@
& windeployqt --release --qmldir src\qml --qmldir $imports --no-system-d3d-compiler --no-opengl-sw --no-compiler-runtime (Join-Path $out 'instacache.exe')
if ($LASTEXITCODE -ne 0) {
    throw "windeployqt failed with $LASTEXITCODE"
}

# The Visual C++ runtime, beside the executable, which Microsoft allows: a
# machine that never installed the redistributable would otherwise refuse to
# start instaCache with a missing vcruntime140.dll.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products * -property installationPath
$crt = Get-ChildItem -Path (Join-Path $vs 'VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT') -Directory |
    Sort-Object FullName | Select-Object -Last 1
if (-not $crt) {
    throw "the Visual C++ runtime was not found under $vs"
}
Copy-Item -Path (Join-Path $crt.FullName '*.dll') -Destination $out

Copy-Item -LiteralPath 'assets\instacache.ico' -Destination $out
foreach ($file in @('install.ps1', 'install.cmd', 'uninstall.ps1', 'get.ps1', 'README.md', 'LICENSE', 'CHANGELOG.md')) {
    Copy-Item -LiteralPath $file -Destination $out
}
Set-Content -LiteralPath (Join-Path $out 'version.txt') -Encoding ascii -Value $Version

$zip = Join-Path $dist "$name.zip"
Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue
& 7z a -tzip -mx=9 $zip $out | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "7z failed with $LASTEXITCODE"
}
$hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
# LF, and the two spaces sha256sum puts between the hash and the name.
[IO.File]::WriteAllText("$zip.sha256", "$hash  $name.zip`n")

Get-ChildItem -LiteralPath $dist -File | Format-Table Name, Length -AutoSize | Out-String | Write-Host
if ($env:GITHUB_OUTPUT) {
    Add-Content -LiteralPath $env:GITHUB_OUTPUT -Value "name=$name"
}
