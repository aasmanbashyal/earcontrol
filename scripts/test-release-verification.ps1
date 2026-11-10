$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/pe-check.ps1"
$scratch = Join-Path ([IO.Path]::GetTempPath()) ("earcontrol-pe-check-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
function New-Fixture([string]$Name, [string]$Import = '', [string]$DelayImport = '') {
    $bytes = [byte[]]::new(1536)
    function Put16([int]$Offset, [uint16]$Value) { [BitConverter]::GetBytes($Value).CopyTo($bytes, $Offset) }
    function Put32([int]$Offset, [uint32]$Value) { [BitConverter]::GetBytes($Value).CopyTo($bytes, $Offset) }
    Put16 0 0x5a4d
    Put32 0x3c 0x80
    Put32 0x80 0x4550
    Put16 0x84 0x8664
    Put16 0x86 1
    Put16 0x94 240
    Put16 0x98 0x20b
    Put16 (0x98 + 68) 2
    Put32 (0x98 + 108) 16
    # One file-backed section begins at RVA 0x1000 / file offset 0x200.
    Put32 (0x188 + 12) 0x1000
    Put32 (0x188 + 16) 0x400
    Put32 (0x188 + 20) 0x200
    if ($Import) {
        Put32 (0x98 + 112 + 8) 0x1000
        Put32 (0x98 + 112 + 12) 40
        Put32 (0x200 + 12) 0x1060
        [Text.Encoding]::ASCII.GetBytes($Import).CopyTo($bytes, 0x260)
    }
    if ($DelayImport) {
        Put32 (0x98 + 112 + 13 * 8) 0x1100
        Put32 (0x98 + 112 + 13 * 8 + 4) 64
        Put32 0x300 1
        Put32 0x304 0x1160
        [Text.Encoding]::ASCII.GetBytes($DelayImport).CopyTo($bytes, 0x360)
    }
    $path = Join-Path $scratch $Name
    [IO.File]::WriteAllBytes($path, $bytes)
    $path
}
function Assert-Rejected([string]$Path, [string]$Message) {
    $rejected = $false
    try { Assert-PortableApp $Path } catch { $rejected = $_.Exception.Message -like "*$Message*" }
    if (!$rejected) { throw "Expected rejection containing '$Message': $Path" }
}
try {
    $valid = New-Fixture 'valid.exe' 'KERNEL32.dll' 'USER32.dll'
    Assert-PortableApp $valid
    $info = Get-PeInfo $valid
    if ($info.Imports.Count -ne 2 -or $info.Imports -notcontains 'USER32.dll') { throw 'Delay imports were not parsed.' }
    Assert-Rejected (New-Fixture 'dynamic.exe' 'VCRUNTIME140.dll') 'runtime DLLs'
    Assert-Rejected (New-Fixture 'delayed.exe' '' 'vcruntime140_1.dll') 'runtime DLLs'
    $wrongArch = New-Fixture 'x86.exe'
    $bytes = [IO.File]::ReadAllBytes($wrongArch)
    [BitConverter]::GetBytes([uint16]0x14c).CopyTo($bytes, 0x84)
    [IO.File]::WriteAllBytes($wrongArch, $bytes)
    Assert-Rejected $wrongArch 'x64 Windows GUI'
    $wrongPe = Join-Path $scratch 'invalid.exe'
    [IO.File]::WriteAllBytes($wrongPe, [byte[]]::new(64))
    Assert-Rejected $wrongPe 'Not a Windows executable'
    $truncated = Join-Path $scratch 'truncated.exe'
    [IO.File]::WriteAllBytes($truncated, ([IO.File]::ReadAllBytes($valid))[0..127])
    Assert-Rejected $truncated 'Invalid PE file range'
    $badRva = New-Fixture 'bad-rva.exe' 'KERNEL32.dll'
    $bytes = [IO.File]::ReadAllBytes($badRva)
    [BitConverter]::GetBytes([uint32]0x2000).CopyTo($bytes, 0x20c)
    [IO.File]::WriteAllBytes($badRva, $bytes)
    Assert-Rejected $badRva 'outside a file-backed section'
    Write-Output 'Passed 7 portable PE checks, including ordinary and delay-loaded runtime imports.'
} finally {
    $resolved = [IO.Path]::GetFullPath($scratch)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
    if ((Split-Path -Parent $resolved) -ne $tempRoot -or (Split-Path -Leaf $resolved) -notlike 'earcontrol-pe-check-*') {
        throw 'Refusing to remove a path outside the test scratch directory.'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
