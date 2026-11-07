param([ValidateSet('Check', 'Build', 'Dev', 'Test', 'WindowsTest', 'Package')][string]$Task = 'Build')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$localCargo = Join-Path $projectRoot '.tools\cargo'
if (Test-Path -LiteralPath (Join-Path $localCargo 'bin\cargo.exe')) {
    $env:CARGO_HOME = $localCargo
    $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
    $env:PATH = "$localCargo\bin;$env:PATH"
}
Push-Location (Join-Path $projectRoot 'desktop')
try {
    $releaseVersion = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
    $tauriVersion = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
    $cargoManifest = Get-Content -LiteralPath 'src-tauri/Cargo.toml' -Raw
    $cargoVersion = [regex]::Match($cargoManifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    if ($releaseVersion -ne $tauriVersion -or $releaseVersion -ne $cargoVersion) {
        throw 'Versions in package.json, Cargo.toml, and tauri.conf.json must match.'
    }
    if (!(Test-Path -LiteralPath 'node_modules')) {
        & bun install --frozen-lockfile
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    switch ($Task) {
        'Check' {
            & bun run build
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            & cargo check --locked --manifest-path src-tauri/Cargo.toml
        }
        'Build' { & bun run tauri build --no-bundle -- --locked }
        'Dev' { & bun run tauri dev }
        'Test' {
            & bun test src
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            & bun run build
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            & cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
        }
        'WindowsTest' { & cargo test --locked --manifest-path src-tauri/Cargo.toml native_windows_smoke -- --ignored --test-threads=1 --nocapture }
        'Package' { & bun run tauri build --ci --bundles nsis -- --locked }
    }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($Task -in @('Build', 'Package')) {
        $portableDirectory = Join-Path $projectRoot "dist\EarControl-$releaseVersion"
        # A fresh portable folder prevents stale files from entering the ZIP.
        if (Test-Path -LiteralPath $portableDirectory) {
            $resolvedPortable = (Resolve-Path -LiteralPath $portableDirectory).Path
            $distRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist'))
            if ((Split-Path -Parent $resolvedPortable) -ne $distRoot) { throw 'Portable directory must be inside dist.' }
            Remove-Item -LiteralPath $resolvedPortable -Recurse -Force
        }
        New-Item -ItemType Directory -Path $portableDirectory -Force | Out-Null
        Copy-Item -LiteralPath 'src-tauri\target\release\earcontrol.exe' -Destination (Join-Path $portableDirectory 'EarControl.exe') -Force
        Copy-Item -LiteralPath (Join-Path $projectRoot 'docs\portable-readme.md') -Destination (Join-Path $portableDirectory 'README.md') -Force
        Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination (Join-Path $portableDirectory 'LICENSE') -Force
        $portableZip = Join-Path $projectRoot "dist\EarControl-$releaseVersion-win-x64.zip"
        Compress-Archive -Path (Join-Path $portableDirectory '*') -DestinationPath $portableZip -Force
        $artifacts = @((Join-Path $portableDirectory 'EarControl.exe'), $portableZip)
        if ($Task -eq 'Package') {
            $installerName = "EarControl_${releaseVersion}_x64-setup.exe"
            $installer = Join-Path $projectRoot "dist\$installerName"
            Copy-Item -LiteralPath "src-tauri\target\release\bundle\nsis\$installerName" -Destination $installer -Force
            $artifacts += $installer
        }
        $checksumLines = foreach ($artifact in $artifacts) {
            $relativeArtifact = $artifact.Substring((Join-Path $projectRoot 'dist').Length + 1).Replace('\', '/')
            '{0}  {1}' -f (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant(), $relativeArtifact
        }
        $checksumLines | Set-Content -LiteralPath (Join-Path $projectRoot "dist\EarControl-$releaseVersion-SHA256SUMS.txt") -Encoding ascii
    }
} finally { Pop-Location }
