function Get-PeInfo {
    param([Parameter(Mandatory)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Path))
    function Assert-Range([long]$Offset, [long]$Length) {
        if ($Offset -lt 0 -or $Length -lt 0 -or $Offset + $Length -gt $bytes.Length) {
            throw "Invalid PE file range in $Path."
        }
    }
    function Read-U16([long]$Offset) {
        Assert-Range $Offset 2
        [BitConverter]::ToUInt16($bytes, [int]$Offset)
    }
    function Read-U32([long]$Offset) {
        Assert-Range $Offset 4
        [BitConverter]::ToUInt32($bytes, [int]$Offset)
    }
    if ((Read-U16 0) -ne 0x5a4d) { throw "Not a Windows executable: $Path" }
    $pe = [long](Read-U32 0x3c)
    if ((Read-U32 $pe) -ne 0x4550) { throw "Invalid PE signature: $Path" }
    $coff = $pe + 4
    $optional = $coff + 20
    $optionalSize = Read-U16 ($coff + 16)
    Assert-Range $optional $optionalSize
    $magic = Read-U16 $optional
    $directoryOffset = switch ($magic) {
        0x10b { 96 }
        0x20b { 112 }
        default { throw "Unsupported PE optional header: $Path" }
    }
    if ($optionalSize -lt $directoryOffset) { throw "Truncated PE optional header: $Path" }
    $directoryCount = Read-U32 ($optional + $directoryOffset - 4)
    if ($directoryCount -gt [Math]::Floor(($optionalSize - $directoryOffset) / 8)) {
        throw "Truncated PE data directories: $Path"
    }
    $sectionTable = $optional + $optionalSize
    $sectionCount = Read-U16 ($coff + 2)
    Assert-Range $sectionTable (40 * $sectionCount)
    $sections = @(for ($index = 0; $index -lt $sectionCount; $index++) {
        $offset = $sectionTable + 40 * $index
        [pscustomobject]@{
            Rva = [long](Read-U32 ($offset + 12))
            Size = [long](Read-U32 ($offset + 16))
            Raw = [long](Read-U32 ($offset + 20))
        }
    })
    function Resolve-Rva([long]$Rva, [long]$Length) {
        foreach ($section in $sections) {
            if ($Rva -ge $section.Rva -and $Rva + $Length -le $section.Rva + $section.Size) {
                $offset = $section.Raw + $Rva - $section.Rva
                Assert-Range $offset $Length
                return $offset
            }
        }
        throw "PE import data is outside a file-backed section: $Path"
    }
    function Read-ImportName([long]$Rva) {
        $name = [Collections.Generic.List[byte]]::new()
        for ($index = 0; $index -lt 512; $index++) {
            $value = $bytes[(Resolve-Rva ($Rva + $index) 1)]
            if ($value -eq 0) {
                if ($name.Count -eq 0) { throw "Empty PE import name: $Path" }
                return [Text.Encoding]::ASCII.GetString($name.ToArray())
            }
            $name.Add($value)
        }
        throw "Unterminated PE import name: $Path"
    }
    $imports = [Collections.Generic.List[string]]::new()
    # Ordinary imports and RVA-based delay imports cover the MSVC output used here.
    foreach ($table in @(@{Index=1; Size=20; Name=12}, @{Index=13; Size=32; Name=4})) {
        if ($directoryCount -le $table.Index) { continue }
        $directory = $optional + $directoryOffset + 8 * $table.Index
        $rva = [long](Read-U32 $directory)
        $size = [long](Read-U32 ($directory + 4))
        if ($rva -eq 0 -and $size -eq 0) { continue }
        if ($rva -eq 0 -or $size -lt $table.Size) { throw "Invalid PE import directory: $Path" }
        $terminated = $false
        for ($position = 0; $position + $table.Size -le $size; $position += $table.Size) {
            $offset = Resolve-Rva ($rva + $position) $table.Size
            $nameRva = Read-U32 ($offset + $table.Name)
            if ($nameRva -eq 0) { $terminated = $true; break }
            if ($table.Index -eq 13 -and (Read-U32 $offset) -ne 1) {
                throw "Unsupported non-RVA delay imports: $Path"
            }
            $imports.Add((Read-ImportName $nameRva))
        }
        if (!$terminated) { throw "Unterminated PE import directory: $Path" }
    }
    [pscustomobject]@{
        Machine = Read-U16 $coff
        Magic = $magic
        Subsystem = Read-U16 ($optional + 68)
        Imports = @($imports | Sort-Object -Unique)
    }
}

function Assert-PortableApp {
    param([Parameter(Mandatory)][string]$Path)
    $info = Get-PeInfo -Path $Path
    if ($info.Machine -ne 0x8664 -or $info.Magic -ne 0x20b -or $info.Subsystem -ne 2) {
        throw "Release app must be an x64 Windows GUI executable: $Path"
    }
    $runtime = @($info.Imports | Where-Object { $_ -match '^(vcruntime|msvcp|msvcr|concrt)\d.*\.dll$' })
    if ($runtime.Count) {
        throw "Portable app imports Visual C++ runtime DLLs ($($runtime -join ', ')). Rebuild with the repository's crt-static configuration."
    }
}
