# Starts the packaged Windows build and checks that it really runs.
#
#   scripts/smoke-windows.ps1 -Folder dist\instacache-2.14.0-windows-x86_64 -Version 2.14.0
#
# From the archive's own folder, with no Qt on PATH, so a DLL windeployqt
# missed fails here instead of on somebody's machine. It reads --version,
# starts the window on a local page with a profile of its own, checks that
# the engine wrote its cache and that a second launch is handed to the first,
# then stops it. Nothing reaches Instagram.

param(
    [Parameter(Mandatory = $true)]
    [string]$Folder,
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2

$folder = (Resolve-Path -LiteralPath $Folder).Path
$exe = Join-Path $folder 'instacache.exe'
$work = Join-Path ([IO.Path]::GetTempPath()) ('instacache-smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $work | Out-Null

$out = Join-Path $work 'version.txt'
$saved = $env:PATH
$env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
$run = Start-Process -FilePath $exe -ArgumentList '--version' -Wait -PassThru -NoNewWindow -RedirectStandardOutput $out
$said = Get-Content -LiteralPath $out -Raw
Write-Host $said
if ($run.ExitCode -ne 0 -or $said -notmatch "instaCache $([regex]::Escape($Version))") {
    throw "instacache --version exited with $($run.ExitCode) and said: $said"
}
$env:PATH = $saved

# A page of our own, served from here.
$www = Join-Path $work 'www'
New-Item -ItemType Directory -Force -Path $www | Out-Null
Set-Content -LiteralPath (Join-Path $www 'index.html') -Encoding ascii -Value '<!doctype html><title>smoke</title><p>instaCache smoke test'
$server = Start-Process -FilePath 'python' -ArgumentList '-m', 'http.server', '--bind', '127.0.0.1', '8793', '--directory', $www -PassThru -WindowStyle Hidden

# Only the system's own directories from here: whatever else the runner put
# on PATH, Qt included, must not be what makes this work.
$env:PATH = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"

$env:INSTACACHE_CONFIG_HOME = Join-Path $work 'config'
$env:INSTACACHE_DATA_HOME = Join-Path $work 'data'
$env:INSTACACHE_CACHE_HOME = Join-Path $work 'cache'
$env:QT_FORCE_STDERR_LOGGING = '1'
$profileConfig = Join-Path $env:INSTACACHE_CONFIG_HOME 'profiles\smoke'
New-Item -ItemType Directory -Force -Path $profileConfig | Out-Null
Set-Content -LiteralPath (Join-Path $profileConfig 'config.json') -Encoding ascii -Value (@{
        home_url = 'http://127.0.0.1:8793/'
        internal_domains = @('127.0.0.1')
        auto_update = $false
        tray_icon = $true
    } | ConvertTo-Json)

$log = Join-Path $work 'instacache.log'
$app = Start-Process -FilePath $exe -ArgumentList '--profile', 'smoke' -PassThru -RedirectStandardError $log -RedirectStandardOutput (Join-Path $work 'stdout.txt')
try {
    $cache = Join-Path $env:INSTACACHE_CACHE_HOME 'profiles\smoke'
    $deadline = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $deadline -and -not $app.HasExited) {
        if ((Get-ChildItem -LiteralPath $cache -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0) {
            break
        }
        Start-Sleep -Seconds 2
    }
    if ($app.HasExited) {
        throw "instaCache exited with $($app.ExitCode) instead of showing the page"
    }
    if ((Get-ChildItem -LiteralPath $cache -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count -eq 0) {
        throw 'the engine never wrote to its cache: the page did not load'
    }
    Write-Host 'The window is up and the engine has written its cache.'

    # A second launch of the profile goes to the first and exits at once.
    $second = Start-Process -FilePath $exe -ArgumentList '--profile', 'smoke', 'http://127.0.0.1:8793/' -Wait -PassThru
    if ($second.ExitCode -ne 0) {
        throw "a second launch exited with $($second.ExitCode)"
    }
    Start-Sleep -Seconds 2
    if ($app.HasExited) {
        throw 'the first instance quit when the second launch reached it'
    }
    Write-Host 'A second launch was handed to the running window.'
} finally {
    if (-not $app.HasExited) {
        Stop-Process -Id $app.Id -Force
    }
    Get-Process -Name 'QtWebEngineProcess' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($folder, [StringComparison]::OrdinalIgnoreCase) } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    Write-Host '--- instaCache log ---'
    if (Test-Path -LiteralPath $log) {
        Get-Content -LiteralPath $log | Select-Object -Last 60 | Write-Host
    }
}
