# Build a portable package to hand to a tester.
#
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1
#
# Produces dist\incredibulk-<version>-windows-x64.zip containing the executable and a
# short note. No installer: Incredibulk is a single file, and asking a tester to run
# an installer for one file is friction with nothing behind it.

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$version = (Select-String -Path (Join-Path $root 'app\tauri.conf.json') -Pattern '"version"\s*:\s*"([^"]+)"').Matches[0].Groups[1].Value
$name = "incredibulk-$version-windows-x64"
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name

Write-Host "building Incredibulk $version"
Push-Location $root
try {
  cargo build -p incredibulk --release
  if ($LASTEXITCODE -ne 0) { throw 'the release build failed' }
} finally {
  Pop-Location
}

# The target directory is redirected on this machine, so ask cargo rather than
# assuming it sits under the project.
$targetDir = $env:CARGO_TARGET_DIR
if (-not $targetDir) {
  $configPath = Join-Path $root '.cargo\config.toml'
  if (Test-Path $configPath) {
    $match = Select-String -Path $configPath -Pattern 'target-dir\s*=\s*"([^"]+)"'
    if ($match) { $targetDir = $match.Matches[0].Groups[1].Value }
  }
}
if (-not $targetDir) { $targetDir = Join-Path $root 'target' }

$exe = Join-Path $targetDir 'release\incredibulk.exe'
if (-not (Test-Path $exe)) { throw "no executable at $exe" }

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $exe $stage

$note = @"
Incredibulk $version
===================

An accumulating clipboard. Open a session, copy as much as you need, then paste
all of it as one block.

RUNNING IT
----------
Double click incredibulk.exe. Nothing installs; it is this one file.

Windows will very likely show a blue "Windows protected your PC" box, because
this build is not code signed. Click "More info", then "Run anyway". That is
expected for an unsigned build and is not a sign anything is wrong.

The app has no window of its own. It puts an icon in the system tray, at the
bottom right, next to the clock. It may be hidden behind the small arrow there.
The first launch opens a short introduction you can skip.

THE TWO SHORTCUTS
-----------------
  Ctrl+Alt+C   open a session, then copy as usual with Ctrl+C
  Ctrl+Alt+V   paste everything you collected, as one block

With no session running, Ctrl+Alt+V pastes your last session again.

REQUIREMENTS
------------
Windows 10 or 11 with the Microsoft Edge WebView2 runtime, which is already
present on Windows 11 and on any Windows 10 that has been updated in the last
few years. If the app starts and no tray icon appears, that is the first thing
to check.

REMOVING IT
-----------
Right click the tray icon and choose "Remove Incredibulk". It takes away the login
entry, the settings, and the browser cache it built up, then tells you what is
left to delete by hand. Deleting incredibulk.exe on its own leaves those behind.

REPORTING A PROBLEM
-------------------
If it disappears without a word, look for:

  %APPDATA%\dev.incredibulk.app\crash.log

Settings and saved sessions live in that same folder.
"@
Set-Content -Path (Join-Path $stage 'README.txt') -Value $note -Encoding utf8

$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$stage\*" -DestinationPath $zip

Write-Host ""
Write-Host "package: $zip"
Write-Host ("size:    {0:N1} MB" -f ((Get-Item $zip).Length / 1MB))
Write-Host "contents:"
Get-ChildItem $stage | ForEach-Object { Write-Host ("  {0,-14} {1,10:N0} bytes" -f $_.Name, $_.Length) }
