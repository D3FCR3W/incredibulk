# Assemble the local one-click update beside its versioned installer.
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$InstallerPath,
  [Parameter(Mandatory = $true)][string]$ApplicationPath,
  [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$installer = Get-Item -LiteralPath $InstallerPath
$application = Get-Item -LiteralPath $ApplicationPath
$version = $installer.VersionInfo.ProductVersion
if ($version -notmatch '^\d+\.\d+\.\d+$' -or $application.VersionInfo.ProductVersion -ne $version) {
  throw "Installer version '$version' and build version '$($application.VersionInfo.ProductVersion)' must be the same release version."
}

# Tauri patches the bundle marker (and may sign the executable) while bundling,
# then restores the unpatched build output. Hash the payload that NSIS installs.
$sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
if (-not $sevenZip) {
  $sevenZip = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) |
    Where-Object { $_ } |
    ForEach-Object { Join-Path $_ '7-Zip\7z.exe' } |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
}
if (-not $sevenZip) { throw 'Install 7-Zip to verify the application inside the NSIS installer.' }

$extracted = Join-Path ([IO.Path]::GetTempPath()) ('incredibulk-package-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $extracted | Out-Null
try {
  & $sevenZip e -y "-o$extracted" $installer.FullName 'incredibulk.exe' | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Could not extract the application from the installer.' }
  $payload = Get-Item -LiteralPath (Join-Path $extracted 'incredibulk.exe') -ErrorAction Stop
  if ($payload.VersionInfo.ProductVersion -ne $version) {
    throw 'The application inside the installer has a different version.'
  }
  $applicationHash = (Get-FileHash -LiteralPath $payload.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
} finally {
  Remove-Item -LiteralPath $extracted -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $OutputDirectory $installer.Name) -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'update-windows.ps1') -Destination $OutputDirectory -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'update-windows.cmd') -Destination (Join-Path $OutputDirectory 'Mettre-a-jour-Incredibulk.cmd') -Force
[ordered]@{
  version = $version
  installer = $installer.Name
  sha256 = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  applicationSha256 = $applicationHash
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'update.json') -Encoding UTF8
Write-Host "One-click update: $(Join-Path $OutputDirectory 'Mettre-a-jour-Incredibulk.cmd')"
