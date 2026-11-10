param([switch]$VersionOnly, [switch]$PortableOnly)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = [regex]::Match((Get-Content "$projectRoot/desktop/Cargo.toml" -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
$lockedVersion = [regex]::Match((Get-Content "$projectRoot/Cargo.lock" -Raw), '\[\[package\]\]\s+name = "earcontrol"\s+version = "([^"]+)"').Groups[1].Value
if ($version -notmatch '^\d+\.\d+\.\d+$' -or $version -ne $lockedVersion) { throw 'Versions in desktop/Cargo.toml and Cargo.lock must agree.' }
if ($env:GITHUB_REF_TYPE -eq 'tag' -and $env:GITHUB_REF_NAME -ne "v$version") { throw "Tag must equal v$version." }
if ($VersionOnly) { Write-Output "Release version $version verified."; return }
$expected = @("EarControl-$version/EarControl.exe", "EarControl-$version-win-x64.zip")
if (!$PortableOnly) { $expected += "EarControl_${version}_x64-setup.exe" }
$lines = @(Get-Content "$projectRoot/dist/EarControl-$version-SHA256SUMS.txt")
if ($lines.Count -ne $expected.Count) { throw "Expected $($expected.Count) release checksums." }
foreach ($name in $expected) {
    $line = @($lines | Where-Object { $_.EndsWith("  $name") })
    if ($line.Count -ne 1 -or $line[0] -notmatch '^[a-f0-9]{64}  ') { throw "Missing or invalid checksum: $name" }
    $path = Join-Path "$projectRoot/dist" $name
    if ((Get-Item -LiteralPath $path).Length -eq 0) { throw "Empty artifact: $name" }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $line[0].Substring(0,64)) { throw "Checksum mismatch: $name" }
}
$archive = [IO.Compression.ZipFile]::OpenRead((Join-Path "$projectRoot/dist" "EarControl-$version-win-x64.zip"))
try {
    $files = @($archive.Entries | Where-Object { $_.Name -ne '' })
    $archiveExpected = @("EarControl-$version/EarControl.exe", "EarControl-$version/README.md", "EarControl-$version/LICENSE")
    if ($files.Count -ne $archiveExpected.Count) { throw 'Unexpected portable ZIP contents.' }
    foreach ($name in $archiveExpected) {
        $entry = $files | Where-Object { $_.FullName.Replace('\','/') -eq $name }
        if (!$entry -or $entry.Length -eq 0) { throw "Missing or empty ZIP entry: $name" }
    }
    $exe = $files | Where-Object { $_.Name -eq 'EarControl.exe' }
    $stream = $exe.Open()
    try { $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant() }
    finally { $stream.Dispose() }
    $exeLine = $lines | Where-Object { $_.EndsWith("  EarControl-$version/EarControl.exe") }
    if ($hash -ne $exeLine.Substring(0,64)) { throw 'ZIP executable differs from the release executable.' }
} finally { $archive.Dispose() }
Write-Output "Verified version $version, portable ZIP contents, and $($expected.Count) release artifacts."
