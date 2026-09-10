# Build and verify an x64 Windows installer without installing it.
# Run from Windows: powershell -ExecutionPolicy Bypass -File tools\installer.ps1
[CmdletBinding()]
param([string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Run this script on Windows.' }

$root = Split-Path -Parent $PSScriptRoot
$config = Get-Content (Join-Path $root 'app\tauri.conf.json') -Raw | ConvertFrom-Json
$version = $config.version
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'dist' }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

# A native Windows target directory also supports source trees opened over WSL.
$previousTarget = $env:CARGO_TARGET_DIR
if (-not $env:CARGO_TARGET_DIR) {
  $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'Incredibulk\build-target'
}

Push-Location $root
try {
  $compiler = rustc -vV
  if ($LASTEXITCODE -ne 0 -or -not ($compiler -match '^host: x86_64-pc-windows-msvc$')) {
    throw 'The x64 MSVC Rust toolchain is required.'
  }
  cargo tauri --version
  if ($LASTEXITCODE -ne 0) { throw 'Install the bundler with: cargo install tauri-cli --locked' }

  cargo clippy --workspace --all-targets --locked -- -D warnings
  if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
  cargo test --workspace --locked
  if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
  & (Join-Path $PSScriptRoot 'update-windows.tests.ps1')

  $metadata = cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }

  Push-Location (Join-Path $root 'app')
  try {
    cargo tauri build --bundles nsis --ci -- --locked
    if ($LASTEXITCODE -ne 0) { throw 'The installer build failed.' }
  } finally {
    Pop-Location
  }

  $filename = "Incredibulk_${version}_x64-setup.exe"
  $installer = Join-Path $metadata.target_directory "release\bundle\nsis\$filename"
  if (-not (Test-Path $installer)) { throw "Installer not found: $installer" }

  New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
  $destination = Join-Path $OutputDirectory $filename
  Copy-Item $installer $destination -Force
  $hash = (Get-FileHash $destination -Algorithm SHA256).Hash.ToLowerInvariant()
  Set-Content -Path "$destination.sha256" -Value "$hash  $filename" -Encoding ascii
  Write-Host "Installer: $destination"
  Write-Host "SHA256: $hash"
  & (Join-Path $PSScriptRoot 'package-update.tests.ps1')
  & (Join-Path $PSScriptRoot 'package-update.ps1') `
    -InstallerPath $destination `
    -ApplicationPath (Join-Path $metadata.target_directory 'release\incredibulk.exe') `
    -OutputDirectory (Join-Path $OutputDirectory "Incredibulk-update-$version")
} finally {
  Pop-Location
  $env:CARGO_TARGET_DIR = $previousTarget
}
