# Regression for the real NSIS payload versus the unpatched build executable.
# Compiles small fixtures and extracts them; it never runs an installer.
$ErrorActionPreference = 'Stop'
$makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
if (-not $makensis) { $makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe' }
if (-not (Test-Path -LiteralPath $makensis)) { throw 'Build an NSIS installer with Tauri first to install its compiler.' }

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('incredibulk-package-tests-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
try {
  $raw = Join-Path $fixtureRoot 'raw.exe'
  $payload = Join-Path $fixtureRoot 'payload.exe'
  Add-Type -TypeDefinition @'
using System.Reflection;
[assembly: AssemblyVersion("0.1.1.0")]
[assembly: AssemblyInformationalVersion("0.1.1")]
public class RawPackageFixture { public static void Main() { System.Console.WriteLine("unpatched"); } }
'@ -OutputAssembly $raw -OutputType ConsoleApplication
  Add-Type -TypeDefinition @'
using System.Reflection;
[assembly: AssemblyVersion("0.1.1.0")]
[assembly: AssemblyInformationalVersion("0.1.1")]
public class BundledPackageFixture { public static void Main() { System.Console.WriteLine("NSIS payload"); } }
'@ -OutputAssembly $payload -OutputType ConsoleApplication
  $fixtureInstaller = Join-Path $fixtureRoot 'Incredibulk_0.1.1_x64-setup.exe'
  $definition = Join-Path $fixtureRoot 'fixture.nsi'
  @"
Unicode true
!include "MUI2.nsh"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Name "Incredibulk packaging test"
OutFile "$fixtureInstaller"
RequestExecutionLevel user
VIProductVersion "0.1.1.0"
VIAddVersionKey "ProductName" "Incredibulk packaging test"
VIAddVersionKey "FileDescription" "Packaging regression fixture"
VIAddVersionKey "FileVersion" "0.1.1"
VIAddVersionKey "LegalCopyright" ""
VIAddVersionKey "ProductVersion" "0.1.1"
Section
  SetOutPath "`$INSTDIR"
  File "/oname=incredibulk.exe" "$payload"
SectionEnd
"@ | Set-Content -LiteralPath $definition -Encoding UTF8
  & $makensis /V1 $definition
  if ($LASTEXITCODE -ne 0) { throw 'Could not compile the NSIS test fixture.' }
  $destination = Join-Path $fixtureRoot 'update package'
  & (Join-Path $PSScriptRoot 'package-update.ps1') -InstallerPath $fixtureInstaller -ApplicationPath $raw -OutputDirectory $destination
  $manifest = Get-Content -LiteralPath (Join-Path $destination 'update.json') -Raw | ConvertFrom-Json
  if ($manifest.applicationSha256 -ne (Get-FileHash -LiteralPath $payload).Hash) { throw 'Manifest does not describe the packaged executable.' }
  if ($manifest.applicationSha256 -eq (Get-FileHash -LiteralPath $raw).Hash) { throw 'Regression: the raw build executable was used for verification.' }
  if ($manifest.sha256 -ne (Get-FileHash -LiteralPath $fixtureInstaller).Hash) { throw 'Installer checksum differs.' }
  Write-Host 'PASS: a real NSIS archive supplies the application checksum, even when the raw build has the same version but different bytes.'
} finally {
  Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
}
