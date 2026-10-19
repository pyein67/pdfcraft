# Read-only PE/Win32 inspection of the last working and reported-regressing builds.
param([Parameter(Mandatory)][string]$Executable, [Parameter(Mandatory)][string]$Report)
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path -LiteralPath $Executable).Path
[byte[]]$pe = [IO.File]::ReadAllBytes($Exe)
function U16([byte[]]$B, [int]$Offset) { [BitConverter]::ToUInt16($B, $Offset) }
function U32([byte[]]$B, [int]$Offset) { [BitConverter]::ToUInt32($B, $Offset) }
$header = U32 $pe 0x3c
if ((U16 $pe ($header + 4)) -ne 0x8664) { throw 'EXE is not Windows x64.' }
$optional = $header + 24
if ((U16 $pe $optional) -ne 0x20b) { throw 'EXE is not PE32+.' }
$resourceRva = U32 $pe ($optional + 112 + 16)
$sections = $optional + (U16 $pe ($header + 20))
function File-Offset([uint32]$Rva) {
  for ($i = 0; $i -lt (U16 $pe ($header + 6)); $i++) {
    $s = $sections + 40 * $i
    $va = U32 $pe ($s + 12)
    $size = [Math]::Max((U32 $pe ($s + 8)), (U32 $pe ($s + 16)))
    if ($Rva -ge $va -and $Rva -lt ([long]$va + $size)) { return (U32 $pe ($s + 20)) + $Rva - $va }
  }
  throw "Unmapped PE RVA $Rva"
}
$base = File-Offset $resourceRva
function Resource-Entry([int]$Directory, [int]$Id) {
  $n = (U16 $pe ($base + $Directory + 12)) + (U16 $pe ($base + $Directory + 14))
  for ($i = 0; $i -lt $n; $i++) {
    $entry = $base + $Directory + 16 + 8 * $i
    if ((U32 $pe $entry) -eq $Id) { return (U32 $pe ($entry + 4)) -band 0x7fffffff }
  }
  throw "Missing PE resource $Id in directory $Directory"
}
function Resource-Bytes([int]$Type, [int]$Id) {
  $typeDir = Resource-Entry 0 $Type
  $languageDir = Resource-Entry $typeDir $Id
  # First language entry points to IMAGE_RESOURCE_DATA_ENTRY.
  $dataEntry = (U32 $pe ($base + $languageDir + 20)) -band 0x7fffffff
  $start = File-Offset (U32 $pe ($base + $dataEntry))
  $size = U32 $pe ($base + $dataEntry + 4)
  return ,([byte[]]$pe[$start..($start + $size - 1)])
}

$groups = @()
foreach ($id in @(1, 2)) {
  $group = Resource-Bytes 14 $id
  $entries = @()
  for ($i = 0; $i -lt (U16 $group 4); $i++) {
    $entry = 6 + 14 * $i
    $image = Resource-Bytes 3 (U16 $group ($entry + 12))
    $entries += [ordered]@{ Width = $(if ($group[$entry]) { $group[$entry] } else { 256 }); Planes = (U16 $group ($entry + 4)); Bits = (U16 $group ($entry + 6)); Bytes = $image.Length; SHA256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($image)).ToLowerInvariant() }
  }
  $groups += [ordered]@{ Id = $id; Images = $entries }
}
$imports = @()
$importRva = U32 $pe ($optional + 112 + 8)
if ($importRva) {
  $directory = File-Offset $importRva
  for ($i = 0; $i -lt 256; $i++) {
    $nameRva = U32 $pe ($directory + 20 * $i + 12)
    if (!$nameRva) { break }
    $offset = File-Offset $nameRva
    $length = 0
    while ($length -lt 4096 -and $pe[$offset + $length]) { $length++ }
    if ($length -eq 4096) { throw 'Unterminated import name' }
    $imports += [Text.Encoding]::ASCII.GetString($pe, $offset, $length)
  }
}
$manifest = Resource-Bytes 24 1
Add-Type -AssemblyName System.Drawing
$loaded = [Drawing.Icon]::ExtractAssociatedIcon($Exe)
if (!$loaded) { throw 'Win32 icon extraction failed.' }
try { $bitmap = $loaded.ToBitmap(); $size = "$($bitmap.Width)x$($bitmap.Height)"; $bitmap.Dispose() } finally { $loaded.Dispose() }
$info = (Get-Item -LiteralPath $Exe).VersionInfo
[ordered]@{
  SHA256 = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()
  Machine = ('0x{0:X4}' -f (U16 $pe ($header + 4)))
  Subsystem = U16 $pe ($optional + 68)
  SubsystemVersion = "$((U16 $pe ($optional + 48))).$((U16 $pe ($optional + 50)))"
  DllCharacteristics = ('0x{0:X4}' -f (U16 $pe ($optional + 70)))
  Imports = @($imports | Sort-Object)
  FileVersion = $info.FileVersion
  ProductName = $info.ProductName
  ManifestSHA256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($manifest)).ToLowerInvariant()
  Win32LoadedIcon = $size
  IconGroups = $groups
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Report -Encoding utf8
Get-Content -LiteralPath $Report
