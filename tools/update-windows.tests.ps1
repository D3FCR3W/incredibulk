# Filesystem regression tests. The real installer, registry and app are never used.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'update-windows.ps1')
$realInstaller = ${function:Invoke-IncredibulkInstaller}

function Assert-True { param([bool]$Condition, [string]$Message) if (-not $Condition) { throw $Message } }
function Assert-Throws {
  param([scriptblock]$Action, [string]$Pattern)
  $failed = $false
  try { & $Action } catch {
    $failed = $true
    Assert-True ($_.Exception.Message -match $Pattern) "Unexpected error: $($_.Exception.Message)"
  }
  Assert-True $failed 'Expected the operation to fail.'
}

$script:testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('incredibulk-update-tests-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $script:testRoot | Out-Null
$script:passed = 0

function Reset-Fixture {
  $script:fixture = Join-Path $script:testRoot ([guid]::NewGuid().ToString('N'))
  $script:data = Join-Path $script:fixture 'data'
  $script:package = Join-Path $script:fixture 'package'
  $script:installed = Join-Path $script:fixture 'installed app'
  $script:backups = Join-Path $script:fixture 'backups'
  foreach ($directory in @($script:data, $script:package, $script:installed)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
  }
  [IO.File]::WriteAllText((Join-Path $script:data 'config.json'), '{"onboarded":true,"behavior":{"keep_history":true}}')
  [IO.File]::WriteAllText((Join-Path $script:data 'history.json'), '{"entries":[{"name":"kept","items":[{"text":"a quote"}]}]}')
  $script:before = @{}
  foreach ($name in @('config.json', 'history.json')) { $script:before[$name] = (Get-FileHash (Join-Path $script:data $name)).Hash }
  [IO.File]::WriteAllText((Join-Path $script:installed 'incredibulk.exe'), 'old application')
  $installer = Join-Path $script:package 'Incredibulk_0.1.1_x64-setup.exe'
  [IO.File]::WriteAllText($installer, 'fake installer; never executed')
  $reference = Join-Path $script:fixture 'expected.exe'
  [IO.File]::WriteAllText($reference, 'new application')
  @{
    version = '0.1.1'; installer = 'Incredibulk_0.1.1_x64-setup.exe'
    sha256 = (Get-FileHash $installer).Hash
    applicationSha256 = (Get-FileHash $reference).Hash
  } | ConvertTo-Json | Set-Content (Join-Path $script:package 'update.json') -Encoding UTF8
  $script:fakeVersion = [version]'0.1.0'
  $script:mode = 'normal'
  $script:installerCalls = 0
  $script:restartCalls = 0
  $script:running = $false
  $script:waitPolls = 0
  $script:sleepCalls = 0
}

# Replace the OS boundary functions, leaving backups and integrity checks real.
function Get-IncredibulkInstallation {
  [pscustomobject]@{ Directory = $script:installed; Executable = (Join-Path $script:installed 'incredibulk.exe'); Version = $script:fakeVersion }
}
function Get-IncredibulkDataDirectory { $script:data }
function Get-IncredibulkBackupRoot { $script:backups }
function Get-IncredibulkProcesses {
  if ($script:running) { [pscustomobject]@{ Id = 123 } }
}
function Start-Sleep {
  param([int]$Seconds)
  $script:sleepCalls++
  $script:running = $false
}
function Invoke-IncredibulkInstaller {
  param([string]$Installer, [string]$InstallDirectory)
  Assert-True (-not $script:running) 'An open app must never reach the silent installer.'
  Assert-True ($InstallDirectory -eq $script:installed) 'Installer must keep the existing location.'
  $script:installerCalls++
  if ($script:mode -in @('erase', 'fail')) {
    Remove-Item -LiteralPath (Join-Path $script:data 'history.json')
    [IO.File]::WriteAllText((Join-Path $script:data 'config.json'), '{}')
  }
  if ($script:mode -eq 'fail') { return 7 }
  [IO.File]::WriteAllText((Join-Path $script:installed 'incredibulk.exe'), 'new application')
  $script:fakeVersion = [version]'0.1.1'
  if ($script:mode -eq 'wrong-binary') { [IO.File]::WriteAllText((Join-Path $script:installed 'incredibulk.exe'), 'wrong binary') }
  return 0
}
function Start-Incredibulk { param([string]$Executable) $script:restartCalls++ }
function Assert-DataPreserved {
  foreach ($name in @('config.json', 'history.json')) {
    Assert-True ((Get-FileHash (Join-Path $script:data $name)).Hash -eq $script:before[$name]) "$name changed."
  }
}
function Pass { param([string]$Name) $script:passed++; Write-Host "PASS: $Name" }

try {
  Reset-Fixture
  Invoke-IncredibulkUpdate $script:package
  Assert-DataPreserved
  Assert-True ($script:installerCalls -eq 1 -and $script:restartCalls -eq 1) 'Normal update must install and restart.'
  Assert-True (@(Get-ChildItem $script:backups -Directory).Count -eq 1) 'A backup must remain after success.'
  Pass 'normal update retains settings/history and a backup'

  Reset-Fixture
  $script:mode = 'erase'
  Invoke-IncredibulkUpdate $script:package
  Assert-DataPreserved
  Assert-True ($script:restartCalls -eq 1) 'Verified restoration should allow restart.'
  Pass 'deleted history and overwritten settings are restored byte for byte'

  Reset-Fixture
  $script:mode = 'fail'
  Assert-Throws { Invoke-IncredibulkUpdate $script:package } 'code 7'
  Assert-DataPreserved
  Assert-True ($script:restartCalls -eq 0) 'Do not launch after an installation failure.'
  Pass 'failed installation still restores the data'

  Reset-Fixture
  [IO.File]::WriteAllText((Join-Path $script:package 'Incredibulk_0.1.1_x64-setup.exe'), 'corrupt')
  Assert-Throws { Invoke-IncredibulkUpdate $script:package } 'incomplet'
  Assert-True ($script:installerCalls -eq 0 -and -not (Test-Path $script:backups)) 'Corrupt packages must fail before any writes.'
  Pass 'corrupt installer is rejected before backup or installation'

  Reset-Fixture
  $script:fakeVersion = [version]'0.2.0'
  Assert-Throws { Invoke-IncredibulkUpdate $script:package } 'recente'
  Assert-True ($script:installerCalls -eq 0) 'Downgrades must never execute.'
  Pass 'downgrades are refused'

  Reset-Fixture
  $script:mode = 'wrong-binary'
  Assert-Throws { Invoke-IncredibulkUpdate $script:package } 'empreinte'
  Assert-DataPreserved
  Assert-True ($script:restartCalls -eq 0) 'Do not start an unverified binary.'
  Pass 'installed executable integrity is checked'

  Reset-Fixture
  $script:running = $true
  Invoke-IncredibulkUpdate $script:package
  Assert-True ($script:sleepCalls -eq 1 -and $script:installerCalls -eq 1) 'Wait for the app to exit before installing.'
  Pass 'running app is waited for, never killed'

  Reset-Fixture
  $script:running = $true
  Assert-Throws { Wait-IncredibulkClosed -TimeoutSeconds 0 } 'toujours ouverte'
  Assert-True ($script:running -and $script:installerCalls -eq 0) 'A timeout must leave the running app alone.'
  Pass 'timeout leaves a running session untouched'

  Reset-Fixture
  Invoke-IncredibulkUpdate $script:package -ValidateOnly
  Assert-True ($script:installerCalls -eq 0 -and $script:restartCalls -eq 0 -and -not (Test-Path $script:backups)) 'Validation must be read only.'
  Pass 'check-only does not modify files or processes'

  Reset-Fixture
  Invoke-IncredibulkUpdate $script:package
  $script:running = $true
  Invoke-IncredibulkUpdate $script:package
  Assert-True ($script:installerCalls -eq 1 -and $script:restartCalls -eq 1) 'An identical version must not be reinstalled or restarted.'
  Pass 'repeat launch is harmless when already updated'

  Reset-Fixture
  Remove-Item -LiteralPath $script:data -Recurse
  Invoke-IncredibulkUpdate $script:package
  Assert-True (-not (Test-Path $script:data)) 'A user with no saved files should not gain fake data.'
  Pass 'missing data directory is supported'

  Reset-Fixture
  $backup = Join-Path $script:fixture 'manual-backup'
  $records = Backup-IncredibulkData $script:data $backup
  [IO.File]::WriteAllText((Join-Path $backup 'history.json'), 'damaged backup')
  Remove-Item -LiteralPath (Join-Path $script:data 'history.json')
  Assert-Throws { Restore-IncredibulkDataIfChanged $script:data $backup $records } 'alteree'
  Assert-True (-not (Test-Path (Join-Path $script:data 'history.json'))) 'Never replace data with a damaged backup.'
  Pass 'damaged backups are rejected'

  Reset-Fixture
  $locked = [IO.File]::Open((Join-Path $script:data 'config.json'), [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  try {
    Assert-Throws { Invoke-IncredibulkUpdate $script:package } '.'
    Assert-True ($script:installerCalls -eq 0) 'Never install if a backup cannot be read.'
  } finally { $locked.Dispose() }
  Assert-DataPreserved
  Pass 'backup failure prevents installation'

  Reset-Fixture
  $backup = Join-Path $script:fixture 'manual-backup'
  $records = Backup-IncredibulkData $script:data $backup
  [IO.File]::WriteAllText((Join-Path $script:data 'history.json'), 'new work from a reopened app')
  $newWorkHash = (Get-FileHash (Join-Path $script:data 'history.json')).Hash
  $script:running = $true
  Assert-Throws { Restore-IncredibulkDataIfChanged $script:data $backup $records } 'rouvert'
  Assert-True ((Get-FileHash (Join-Path $script:data 'history.json')).Hash -eq $newWorkHash) 'Never overwrite work from an app that has reopened.'
  Pass 'a reopened app prevents restoration from overwriting its work'

  Reset-Fixture
  $stub = Join-Path $script:fixture 'installer stub.exe'
  Add-Type -TypeDefinition @'
public class InstallerStub {
  public static int Main() {
    System.IO.File.WriteAllText(System.IO.Path.Combine(System.AppDomain.CurrentDomain.BaseDirectory, "arguments.txt"), System.Environment.CommandLine);
    return 7;
  }
}
'@ -OutputAssembly $stub -OutputType ConsoleApplication
  $code = & $realInstaller $stub $script:installed
  Assert-True ($code -eq 7) 'The native installer exit code must reach the caller.'
  $arguments = [IO.File]::ReadAllText((Join-Path $script:fixture 'arguments.txt'))
  Assert-True ($arguments.EndsWith("/S /UPDATE /NS /D=$script:installed")) 'Pass update mode and the complete existing install path, including spaces.'
  Pass 'real process invocation uses silent update mode and returns the exit code'

  Write-Host "$script:passed update regression tests passed."
} finally {
  Remove-Item -LiteralPath $script:testRoot -Recurse -Force
}
