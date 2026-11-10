param(
    [switch]$DisposableVm,
    [string]$PreviousInstaller
)

$ErrorActionPreference = 'Stop'
if (!$DisposableVm) { throw 'Run this test with -DisposableVm in a fresh Windows VM or Windows Sandbox.' }
$root = Split-Path -Parent $PSScriptRoot
& "$PSScriptRoot/verify-release.ps1"
$version = [regex]::Match((Get-Content "$root/desktop/Cargo.toml" -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
$installer = Join-Path "$root/dist" "EarControl_${version}_x64-setup.exe"
$portableExe = Join-Path "$root/dist" "EarControl-$version/EarControl.exe"
$currentDirectory = Join-Path $env:LOCALAPPDATA 'EarControl'
$uninstallKey = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/EarControl'
$installKey = 'HKCU:/Software/EarControl'
$startMenuLink = Join-Path ([Environment]::GetFolderPath('Programs')) 'EarControl.lnk'
$desktopLink = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'EarControl.lnk'
$profile = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'app.earcontrol.desktop/profiles.json'
$fixture = Join-Path $root 'desktop/tests/fixtures/v2.1.2-profiles.json'

function Assert-Absent([string[]]$Paths) {
    foreach ($path in $Paths) {
        if (Test-Path -LiteralPath $path) { throw "Unexpected existing path: $path" }
    }
}

function Assert-Hash([string]$Path, [string]$Expected) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ne $Expected) {
        throw "SHA-256 mismatch: $Path"
    }
}

function Invoke-Setup([string]$Path, [string[]]$Arguments, [int]$ExpectedExitCode = 0) {
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -WindowStyle Hidden -Wait -PassThru
    try {
        if ($process.ExitCode -ne $ExpectedExitCode) {
            throw "$(Split-Path -Leaf $Path) returned $($process.ExitCode), expected $ExpectedExitCode."
        }
    } finally { $process.Dispose() }
}

function Assert-CurrentInstall {
    $app = Join-Path $currentDirectory 'EarControl.exe'
    Assert-Hash $app (Get-FileHash -LiteralPath $portableExe -Algorithm SHA256).Hash
    foreach ($name in @('Uninstall.exe', 'LICENSE', 'README.md', 'THIRD-PARTY-NOTICES.txt')) {
        if ((Get-Item -LiteralPath (Join-Path $currentDirectory $name)).Length -eq 0) {
            throw "Empty installed file: $name"
        }
    }
    $entry = Get-ItemProperty -LiteralPath $uninstallKey
    if ($entry.DisplayVersion -ne $version -or $entry.UninstallString -ne "`"$currentDirectory\Uninstall.exe`"") {
        throw 'Incorrect per-user uninstall registration.'
    }
    if (!(Test-Path -LiteralPath $startMenuLink -PathType Leaf) -or $shell.CreateShortcut($startMenuLink).TargetPath -ne $app) {
        throw 'The Start menu shortcut does not point to the installed application.'
    }
}

function Uninstall-Current {
    Invoke-Setup (Join-Path $currentDirectory 'Uninstall.exe') @('/S')
    Assert-Absent @($currentDirectory, $uninstallKey, $installKey, $startMenuLink, $desktopLink)
}

if (Get-Process earcontrol -ErrorAction SilentlyContinue) { throw 'An EarControl process is already running.' }
Assert-Absent @($currentDirectory, $uninstallKey, $installKey, $startMenuLink, $desktopLink, $profile)
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('EarControl installer test ' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
if (!$PreviousInstaller) {
    $PreviousInstaller = Join-Path $scratch 'EarControl_2.1.2_x64-setup.exe'
    Invoke-WebRequest -Uri 'https://github.com/aasmanbashyal/earcontrol/releases/download/v2.1.2/EarControl_2.1.2_x64-setup.exe' -OutFile $PreviousInstaller -TimeoutSec 90
}
$PreviousInstaller = (Resolve-Path -LiteralPath $PreviousInstaller).Path
Assert-Hash $PreviousInstaller 'af038602b5a966bfc9e02ad55f0d13676917f0a6dcf06a8cf4995b585dc7af5e'
$shell = New-Object -ComObject WScript.Shell

Invoke-Setup $installer @('/S')
Assert-CurrentInstall
Assert-Absent @($profile)
Uninstall-Current
Write-Host "PASS: clean $version install, Start menu target, and uninstall."

$oldDirectory = Join-Path $scratch 'Previous version'
Invoke-Setup $PreviousInstaller @('/S', "/D=$oldDirectory")
$oldApp = Join-Path $oldDirectory 'earcontrol.exe'
$oldHash = (Get-FileHash -LiteralPath $oldApp -Algorithm SHA256).Hash
$oldEntry = Get-ItemProperty -LiteralPath $uninstallKey
if ($oldEntry.DisplayVersion -ne '2.1.2' -or $oldEntry.UninstallString -ne "`"$oldDirectory\uninstall.exe`"") {
    throw 'The published 2.1.2 installer did not register the custom installation.'
}
New-Item -ItemType Directory -Path (Split-Path -Parent $profile) -Force | Out-Null
Copy-Item -LiteralPath $fixture -Destination $profile
$profileHash = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash

Invoke-Setup $installer @('/S') 1
$entryAfterRefusal = Get-ItemProperty -LiteralPath $uninstallKey
if ($entryAfterRefusal.DisplayVersion -ne $oldEntry.DisplayVersion -or $entryAfterRefusal.UninstallString -ne $oldEntry.UninstallString) {
    throw 'The attempted upgrade changed the 2.1.2 uninstall registration.'
}
Assert-Hash $oldApp $oldHash
Assert-Hash $profile $profileHash
Assert-Absent @($currentDirectory)
Write-Host 'PASS: installation over registered 2.1.2 is refused without changing its files, registration or profile.'

Invoke-Setup (Join-Path $oldDirectory 'uninstall.exe') @('/S')
Assert-Absent @($oldApp, $uninstallKey, $startMenuLink, $desktopLink)
Assert-Hash $profile $profileHash
Invoke-Setup $installer @('/S')
Assert-CurrentInstall
Assert-Hash $profile $profileHash
Uninstall-Current
Assert-Hash $profile $profileHash
Write-Host "PASS: 2.1.2 to $version migration and final uninstall preserve the saved profile byte for byte."
