# A manual, local update package. No downloads and no forced process termination.
[CmdletBinding()]
param(
  [string]$PackageDirectory = $PSScriptRoot,
  [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'

function Get-IncredibulkInstallation {
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Incredibulk'
  $entry = Get-ItemProperty -LiteralPath $key -ErrorAction Stop
  $directory = $entry.InstallLocation.Trim('"')
  if (-not [System.IO.Path]::IsPathRooted($directory)) { throw 'Dossier installation invalide.' }
  $executable = Join-Path $directory 'incredibulk.exe'
  if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Application installee introuvable.' }
  if (-not (Test-Path -LiteralPath (Join-Path $directory 'uninstall.exe') -PathType Leaf)) {
    throw 'Ce lanceur attend une installation Incredibulk NSIS existante.'
  }
  [pscustomobject]@{ Directory = $directory; Executable = $executable; Version = [version]$entry.DisplayVersion }
}

function Get-IncredibulkProcesses {
  @(Get-Process -Name incredibulk -ErrorAction SilentlyContinue)
}

function Wait-IncredibulkClosed {
  param([int]$TimeoutSeconds = 1800)
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $announced = $false
  while (@(Get-IncredibulkProcesses).Count -gt 0) {
    if (-not $announced) {
      Write-Host 'Incredibulk est ouvert. Terminez votre session de copie, puis choisissez Quit Incredibulk dans son icone pres de l horloge.' -ForegroundColor Yellow
      Write-Host 'La mise a jour reprendra automatiquement. Aucune fermeture forcee.'
      $announced = $true
    }
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Application toujours ouverte. Relancez ce fichier une fois Incredibulk ferme.' }
    Start-Sleep -Seconds 2
  }
}

function Get-IncredibulkDataDirectory { Join-Path $env:APPDATA 'dev.incredibulk.app' }
function Get-IncredibulkBackupRoot { Join-Path $env:LOCALAPPDATA 'IncredibulkBackups' }

function Get-UpdateManifest {
  param([string]$Directory)
  $manifest = Get-Content -LiteralPath (Join-Path $Directory 'update.json') -Raw | ConvertFrom-Json
  if ($manifest.version -notmatch '^\d+\.\d+\.\d+$' -or
      $manifest.installer -notmatch '^Incredibulk_[0-9.]+_x64-setup\.exe$' -or
      $manifest.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or
      $manifest.applicationSha256 -notmatch '^[a-fA-F0-9]{64}$') {
    throw 'Description du paquet de mise a jour invalide.'
  }
  $installer = Join-Path $Directory $manifest.installer
  if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $manifest.sha256) {
    throw 'Le fichier installation est incomplet ou a change. Rien ne sera installe.'
  }
  $manifest
}

function Backup-IncredibulkData {
  param([string]$DataDirectory, [string]$BackupDirectory)
  New-Item -ItemType Directory -Path $BackupDirectory -ErrorAction Stop | Out-Null
  $records = @()
  foreach ($name in @('config.json', 'history.json')) {
    $original = Join-Path $DataDirectory $name
    if (Test-Path -LiteralPath $original) {
      $hash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
      $copy = Join-Path $BackupDirectory $name
      Copy-Item -LiteralPath $original -Destination $copy -ErrorAction Stop
      if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne $hash) {
        throw "Sauvegarde non verifiable : $name. Installation annulee."
      }
      $records += [pscustomobject]@{ Name = $name; SHA256 = $hash }
    }
  }
  ConvertTo-Json -InputObject $records | Set-Content -LiteralPath (Join-Path $BackupDirectory 'files.json') -Encoding UTF8
  return ,$records
}

function Restore-IncredibulkDataIfChanged {
  param([string]$DataDirectory, [string]$BackupDirectory, [array]$Records)
  # Never overwrite files that a newly opened instance might be changing.
  if (@(Get-IncredibulkProcesses).Count -gt 0) {
    throw "Incredibulk a ete rouvert pendant la mise a jour. Fermez-le avant de restaurer les donnees depuis $BackupDirectory."
  }
  foreach ($record in $Records) {
    $original = Join-Path $DataDirectory $record.Name
    $unchanged = (Test-Path -LiteralPath $original -PathType Leaf) -and
      ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -eq $record.SHA256)
    if (-not $unchanged) {
      $copy = Join-Path $BackupDirectory $record.Name
      if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne $record.SHA256) {
        throw "Sauvegarde alteree : $($record.Name). Restauration interrompue."
      }
      New-Item -ItemType Directory -Force -Path $DataDirectory | Out-Null
      Copy-Item -LiteralPath $copy -Destination $original -Force
      if ((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -ne $record.SHA256) {
        throw "Restauration non verifiable : $($record.Name)."
      }
      Write-Host "Restaure depuis la sauvegarde : $($record.Name)"
    }
  }
}

function Invoke-IncredibulkInstaller {
  param([string]$Installer, [string]$InstallDirectory)
  # NSIS /UPDATE bypasses uninstall and preserves app data and autostart.
  # /D must be last; NSIS treats the complete remaining string as the path.
  $info = New-Object System.Diagnostics.ProcessStartInfo
  $info.FileName = $Installer
  $info.Arguments = "/S /UPDATE /NS /D=$InstallDirectory"
  $info.UseShellExecute = $false
  $process = [System.Diagnostics.Process]::Start($info)
  try {
    $process.WaitForExit()
    return $process.ExitCode
  } finally { $process.Dispose() }
}

function Start-Incredibulk { param([string]$Executable) Start-Process -FilePath $Executable | Out-Null }

function Invoke-IncredibulkUpdate {
  param([string]$Directory, [switch]$ValidateOnly)
  $manifest = Get-UpdateManifest $Directory
  $installation = Get-IncredibulkInstallation
  $targetVersion = [version]$manifest.version
  if ($installation.Version -gt $targetVersion) { throw 'Une version plus recente est deja installee. Retour en arriere refuse.' }
  $dataDirectory = Get-IncredibulkDataDirectory
  if ($ValidateOnly) {
    Write-Host "Verification OK : version $($installation.Version) vers $targetVersion. Aucun fichier modifie."
    return
  }
  if ($installation.Version -eq $targetVersion -and
      (Get-FileHash -LiteralPath $installation.Executable -Algorithm SHA256).Hash -eq $manifest.applicationSha256) {
    Write-Host "Incredibulk $targetVersion est deja a jour."
    if (@(Get-IncredibulkProcesses).Count -eq 0) { Start-Incredibulk $installation.Executable }
    return
  }

  Wait-IncredibulkClosed
  $backupRoot = Get-IncredibulkBackupRoot
  New-Item -ItemType Directory -Force -Path $backupRoot | Out-Null
  $backupDirectory = Join-Path $backupRoot ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
  $records = Backup-IncredibulkData $dataDirectory $backupDirectory
  Write-Host "Sauvegarde verifiee : $backupDirectory"

  try {
    # Recheck the package after waiting for the user, before executing it.
    $currentManifest = Get-UpdateManifest $Directory
    if ($currentManifest.sha256 -ne $manifest.sha256 -or
        $currentManifest.applicationSha256 -ne $manifest.applicationSha256 -or
        $currentManifest.version -ne $manifest.version -or
        $currentManifest.installer -ne $manifest.installer) { throw 'Le paquet a change pendant la preparation.' }
    if (@(Get-IncredibulkProcesses).Count -gt 0) { throw 'Incredibulk a ete rouvert. Relancez la mise a jour apres sa fermeture.' }
    Write-Host "Installation silencieuse de la version $targetVersion..."
    $exitCode = Invoke-IncredibulkInstaller (Join-Path $Directory $manifest.installer) $installation.Directory
    if ($exitCode -ne 0) { throw "Installation echouee (code $exitCode)." }
    $updated = Get-IncredibulkInstallation
    if ($updated.Version -ne $targetVersion) { throw "Version installee $($updated.Version), version attendue $targetVersion." }
    if ($updated.Directory -ne $installation.Directory) { throw 'Le dossier installation a change pendant la mise a jour.' }
    if ((Get-FileHash -LiteralPath $updated.Executable -Algorithm SHA256).Hash -ne $manifest.applicationSha256) {
      throw 'L empreinte du programme installe ne correspond pas au contenu attendu de l installateur.'
    }
  } finally {
    Restore-IncredibulkDataIfChanged $dataDirectory $backupDirectory $records
  }

  Write-Host 'Reglages et historique verifies. Redemarrage...'
  Start-Incredibulk $updated.Executable
  Write-Host "Incredibulk $targetVersion est pret. Sauvegarde conservee : $backupDirectory" -ForegroundColor Green
}

if ($MyInvocation.InvocationName -ne '.') {
  $mutex = $null
  $ownsMutex = $false
  try {
    $mutex = New-Object System.Threading.Mutex($false, 'Local\IncredibulkManualUpdate')
    try { $ownsMutex = $mutex.WaitOne(0) } catch [System.Threading.AbandonedMutexException] { $ownsMutex = $true }
    if (-not $ownsMutex) { throw 'Une mise a jour est deja en cours dans cette session Windows.' }
    Invoke-IncredibulkUpdate -Directory $PackageDirectory -ValidateOnly:$CheckOnly
    exit 0
  } catch {
    Write-Host "Mise a jour interrompue : $($_.Exception.Message)" -ForegroundColor Red
    exit 1
  } finally {
    if ($ownsMutex) { $mutex.ReleaseMutex() }
    if ($null -ne $mutex) { $mutex.Dispose() }
  }
}
