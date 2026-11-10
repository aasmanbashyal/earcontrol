param([ValidateSet('Setup', 'Check', 'Build', 'Dev', 'Test', 'WindowsTest', 'Package', 'Bench')][string]$Task = 'Build')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$localCargo = Join-Path $projectRoot '.tools\cargo'
if (Test-Path -LiteralPath (Join-Path $localCargo 'bin\cargo.exe')) {
    $env:CARGO_HOME = $localCargo
    $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
    $env:PATH = "$localCargo\bin;$env:PATH"
}
function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit code $LASTEXITCODE" }
}
Push-Location $projectRoot
try {
    switch ($Task) {
        'Setup' { Invoke-Cargo fetch --locked }
        'Check' { Invoke-Cargo fmt --all '--' --check; Invoke-Cargo clippy --workspace --all-targets --locked '--' -D warnings }
        'Test' { Invoke-Cargo test --workspace --locked }
        'WindowsTest' { Invoke-Cargo test --locked native_windows_smoke '--' --ignored --test-threads=1 --nocapture }
        'Dev' { Invoke-Cargo run --locked }
        'Bench' { Invoke-Cargo bench --locked --bench workloads }
        default { Invoke-Cargo build --release --locked }
    }
    if ($Task -notin @('Build', 'Package')) { return }
    $releaseVersion = [regex]::Match((Get-Content desktop/Cargo.toml -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
    & "$PSScriptRoot/verify-release.ps1" -VersionOnly
    $portableDirectory = Join-Path $projectRoot "dist\EarControl-$releaseVersion"
    if (Test-Path -LiteralPath $portableDirectory) {
        $resolvedPortable = (Resolve-Path -LiteralPath $portableDirectory).Path
        $distRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist'))
        if ((Split-Path -Parent $resolvedPortable) -ne $distRoot) { throw 'Portable directory must be inside dist.' }
        Remove-Item -LiteralPath $resolvedPortable -Recurse -Force
    }
    New-Item -ItemType Directory -Path $portableDirectory -Force | Out-Null
    Copy-Item -LiteralPath 'target\release\earcontrol.exe' -Destination (Join-Path $portableDirectory 'EarControl.exe')
    Copy-Item -LiteralPath 'docs\portable-readme.md' -Destination (Join-Path $portableDirectory 'README.md')
    Copy-Item -LiteralPath 'LICENSE' -Destination $portableDirectory
    $portableZip = Join-Path $projectRoot "dist\EarControl-$releaseVersion-win-x64.zip"
    Compress-Archive -Path $portableDirectory -DestinationPath $portableZip -Force
    $artifacts = @((Join-Path $portableDirectory 'EarControl.exe'), $portableZip)
    if ($Task -eq 'Package') {
        $nsisCandidates = @(
            (Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'),
            (Join-Path $projectRoot '.tools\nsis\makensis.exe'),
            (Join-Path $env:USERPROFILE 'AppData\Local\tauri\NSIS\makensis.exe')
        )
        if ($env:NSIS_HOME) { $nsisCandidates = @((Join-Path $env:NSIS_HOME 'makensis.exe')) + $nsisCandidates }
        $makensis = $nsisCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if (!$makensis) { $command = Get-Command makensis -ErrorAction SilentlyContinue; if ($command) { $makensis = $command.Source } }
        if (!$makensis) { throw 'Install NSIS 3 or set NSIS_HOME to its directory to build the installer.' }
        & $makensis '/V2' "/DVERSION=$releaseVersion" "/DROOT=$projectRoot" "$PSScriptRoot/installer.nsi"
        if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed.' }
        $artifacts += Join-Path $projectRoot "dist\EarControl_${releaseVersion}_x64-setup.exe"
    }
    $checksumLines = foreach ($artifact in $artifacts) {
        $relativeArtifact = $artifact.Substring((Join-Path $projectRoot 'dist').Length + 1).Replace('\', '/')
        '{0}  {1}' -f (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant(), $relativeArtifact
    }
    $checksumLines | Set-Content -LiteralPath "dist\EarControl-$releaseVersion-SHA256SUMS.txt" -Encoding ascii
} finally { Pop-Location }
