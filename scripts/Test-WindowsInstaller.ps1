[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactsDirectory,

    [ValidateSet('NSIS', 'MSI')]
    [string]$InstallerType = 'NSIS',

    [switch]$RequireSignature,

    [string]$ExpectedSignerThumbprint
)

$ErrorActionPreference = 'Stop'

function Assert-ValidSignature {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedThumbprint
    )

    $signature = Get-AuthenticodeSignature -FilePath $Path
    if ($signature.Status -ne 'Valid') {
        throw "Authenticode signature is not valid for $Path. Status: $($signature.Status)"
    }
    $actualThumbprint = $signature.SignerCertificate.Thumbprint -replace '\s', ''
    $normalizedExpected = $ExpectedThumbprint -replace '\s', ''
    if ($actualThumbprint -ne $normalizedExpected) {
        throw "Authenticode signer does not match the expected Briefli certificate for $Path"
    }
}

function Get-DatabaseSnapshot {
    param([Parameter(Mandatory = $true)][string]$Directory)

    $snapshot = @{}
    Get-ChildItem -Path $Directory -File -Filter 'meeting_minutes.sqlite*' | ForEach-Object {
        $snapshot[$_.Name] = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash
    }
    return $snapshot
}

function Get-BriefliUninstallEntry {
    $uninstallRoots = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )

    foreach ($root in $uninstallRoots) {
        $entry = Get-ItemProperty -Path $root -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -eq 'Briefli' } |
            Select-Object -First 1
        if ($entry) {
            return $entry
        }
    }

    return $null
}

function Split-ExecutableCommand {
    param([Parameter(Mandatory = $true)][string]$Command)

    if ($Command -match '^"([^"]+)"\s*(.*)$') {
        return @($Matches[1], $Matches[2])
    }
    if ($Command -match '^(\S+)\s*(.*)$') {
        return @($Matches[1], $Matches[2])
    }

    throw "Could not parse command: $Command"
}

function Find-BriefliExecutable {
    param([Parameter(Mandatory = $true)]$UninstallEntry)

    $candidates = @()
    if ($UninstallEntry.InstallLocation) {
        $candidates += Join-Path $UninstallEntry.InstallLocation 'Briefli.exe'
    }
    if ($UninstallEntry.DisplayIcon) {
        $candidates += ($UninstallEntry.DisplayIcon -replace '^"|"$|,\d+$', '')
    }
    $candidates += Join-Path $env:LOCALAPPDATA 'Briefli\Briefli.exe'
    $candidates += Join-Path $env:ProgramFiles 'Briefli\Briefli.exe'

    $executable = $candidates |
        Where-Object { $_ -and (Test-Path $_ -PathType Leaf) } |
        Select-Object -First 1
    if (-not $executable) {
        throw 'Briefli.exe was not found after installation.'
    }

    return $executable
}

$installer = if ($InstallerType -eq 'MSI') {
    Get-ChildItem -Path $ArtifactsDirectory -File -Filter '*.msi' | Select-Object -First 1
} else {
    Get-ChildItem -Path $ArtifactsDirectory -File -Filter '*.exe' |
        Where-Object { $_.Name -match '(?i)(setup|installer)' } |
        Select-Object -First 1
}
if (-not $installer) {
    throw "No $InstallerType installer was found in $ArtifactsDirectory"
}

$appDataDirectory = Join-Path $env:APPDATA 'com.briefli.app'
$databasePath = Join-Path $appDataDirectory 'meeting_minutes.sqlite'
if (Get-BriefliUninstallEntry) {
    throw 'Installer smoke test requires a clean profile without an existing Briefli installation.'
}
if (Test-Path $appDataDirectory) {
    $existingDatabaseFiles = Get-ChildItem -Path $appDataDirectory -File -Filter 'meeting_minutes.sqlite*'
    if ($existingDatabaseFiles) {
        throw 'Installer smoke test requires a clean profile without existing Briefli meeting data.'
    }
}

if ($RequireSignature -and -not $ExpectedSignerThumbprint) {
    throw 'ExpectedSignerThumbprint is required when RequireSignature is set.'
}
if ($RequireSignature) {
    Get-ChildItem -Path $ArtifactsDirectory -File |
        Where-Object { $_.Extension -in @('.exe', '.msi') } |
        ForEach-Object {
            Assert-ValidSignature -Path $_.FullName -ExpectedThumbprint $ExpectedSignerThumbprint
        }
}

$install = if ($InstallerType -eq 'MSI') {
    Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', $installer.FullName, '/qn', '/norestart') -Wait -PassThru
} else {
    Start-Process -FilePath $installer.FullName -ArgumentList '/S' -Wait -PassThru
}
if ($install.ExitCode -ne 0) {
    throw "Installer exited with code $($install.ExitCode)"
}

$uninstallEntry = Get-BriefliUninstallEntry
if (-not $uninstallEntry) {
    throw 'Briefli uninstall registration was not found after installation.'
}

$briefliExecutable = Find-BriefliExecutable -UninstallEntry $uninstallEntry
if ($RequireSignature) {
    Assert-ValidSignature -Path $briefliExecutable -ExpectedThumbprint $ExpectedSignerThumbprint
}

New-Item -ItemType Directory -Path $appDataDirectory -Force | Out-Null

$watcher = New-Object System.IO.FileSystemWatcher $appDataDirectory, 'meeting_minutes.sqlite'
$watcher.EnableRaisingEvents = $true
$briefliProcess = $null

try {
    $briefliProcess = Start-Process -FilePath $briefliExecutable -PassThru
    try {
        $inputReady = $briefliProcess.WaitForInputIdle(30000)
    } catch {
        throw "Briefli did not reach an interactive window: $($_.Exception.Message)"
    }

    if (-not $inputReady) {
        throw 'Briefli did not reach an interactive window within 30 seconds.'
    }

    if ($briefliProcess.HasExited) {
        throw "Briefli exited during launch with code $($briefliProcess.ExitCode)"
    }

    if (-not (Test-Path $databasePath -PathType Leaf)) {
        $null = $watcher.WaitForChanged([System.IO.WatcherChangeTypes]::Created, 30000)
    }
    if (-not (Test-Path $databasePath -PathType Leaf)) {
        throw "Briefli did not create its database at $databasePath"
    }
} finally {
    $watcher.Dispose()
    if ($briefliProcess -and -not $briefliProcess.HasExited) {
        Stop-Process -Id $briefliProcess.Id -Force
        Wait-Process -Id $briefliProcess.Id -Timeout 15 -ErrorAction SilentlyContinue
    }
}

$databaseSnapshotBeforeUninstall = Get-DatabaseSnapshot -Directory $appDataDirectory
$uninstallEntry = Get-BriefliUninstallEntry
$uninstallCommand = $uninstallEntry.QuietUninstallString
if (-not $uninstallCommand) {
    $uninstallCommand = $uninstallEntry.UninstallString
}
if (-not $uninstallCommand) {
    throw 'Briefli did not register an uninstall command.'
}

$uninstall = if ($InstallerType -eq 'MSI') {
    $productCode = $uninstallEntry.PSChildName
    if (-not $productCode -or $productCode -notmatch '^\{[0-9A-Fa-f-]+\}$') {
        $productCodeMatch = [regex]::Match($uninstallCommand, '\{[0-9A-Fa-f-]+\}')
        if (-not $productCodeMatch.Success) {
            throw "Could not determine MSI product code from: $uninstallCommand"
        }
        $productCode = $productCodeMatch.Value
    }
    Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/x', $productCode, '/qn', '/norestart') -Wait -PassThru
} else {
    $uninstallParts = Split-ExecutableCommand -Command $uninstallCommand
    $uninstallArguments = $uninstallParts[1]
    if ($uninstallArguments -notmatch '(^|\s)/S(\s|$)') {
        $uninstallArguments = "$uninstallArguments /S".Trim()
    }
    Start-Process -FilePath $uninstallParts[0] -ArgumentList $uninstallArguments -Wait -PassThru
}
if ($uninstall.ExitCode -ne 0) {
    throw "Uninstaller exited with code $($uninstall.ExitCode)"
}

if (Test-Path $briefliExecutable -PathType Leaf) {
    throw "Installed executable remains after uninstall: $briefliExecutable"
}
if ($databaseSnapshotBeforeUninstall.Count -eq 0) {
    throw 'No SQLite database files were available before uninstall.'
}
$databaseSnapshotAfterUninstall = Get-DatabaseSnapshot -Directory $appDataDirectory
foreach ($databaseFile in $databaseSnapshotBeforeUninstall.Keys) {
    if (-not $databaseSnapshotAfterUninstall.ContainsKey($databaseFile)) {
        throw "Uninstall removed local database file $databaseFile"
    }
    if ($databaseSnapshotAfterUninstall[$databaseFile] -ne $databaseSnapshotBeforeUninstall[$databaseFile]) {
        throw "Local database file changed during uninstall: $databaseFile"
    }
}

Write-Host 'Windows installer smoke test passed.'
Write-Host "Installer: $InstallerType - $($installer.Name)"
Write-Host "Database preserved: $databasePath"