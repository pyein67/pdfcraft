# Diagnostic A/B probe ONLY: change RT_GROUP_ICON plane words in a copy of the reported bad EXE.
# No code, PNG pixels, imports, manifest, runtime settings or print implementation is patched.
param([Parameter(Mandatory)][string]$Executable, [Parameter(Mandatory)][string]$Output, [Parameter(Mandatory)][string]$Report)
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path -LiteralPath $Executable).Path
if ([IO.Path]::GetFullPath($Output) -eq $Exe) { throw 'Output must differ from the original executable.' }
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


function Group-Offset([int]$Id) {
  $typeDir = Resource-Entry 0 14
  $languageDir = Resource-Entry $typeDir $Id
  $dataEntry = (U32 $pe ($base + $languageDir + 20)) -band 0x7fffffff
  return File-Offset (U32 $pe ($base + $dataEntry))
}
$original = [byte[]]$pe.Clone()
$changed = @()
foreach ($id in @(1, 2)) {
  $group = Group-Offset $id
  if ((U16 $pe ($group + 4)) -ne 7) { throw 'Expected the reported red build with seven image entries.' }
  for ($i = 0; $i -lt 7; $i++) {
    $offset = $group + 6 + 14 * $i + 4
    if ((U16 $pe $offset) -ne 0) { throw 'Expected the original zero plane word; input was not the reported build.' }
    $pe[$offset] = 1
    $changed += $offset
  }
}
$verification = [byte[]]$pe.Clone()
foreach ($offset in $changed) { $verification[$offset] = $original[$offset] }
$beforeHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($original))
$restoredHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($verification))
if ($restoredHash -cne $beforeHash) { throw 'Bytes other than the declared plane words were changed.' }
$parent = Split-Path ([IO.Path]::GetFullPath($Output)) -Parent
New-Item -ItemType Directory -Force $parent | Out-Null
[IO.File]::WriteAllBytes([IO.Path]::GetFullPath($Output), $pe)
[ordered]@{
  Purpose = 'Metadata-only A/B probe, not a confirmed printing fix'
  InputSHA256 = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()
  OutputSHA256 = (Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant()
  ChangedByteCount = $changed.Count
  ChangedFileOffsets = $changed
  PhysicalPrintingVerified = $false
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Report -Encoding utf8
Get-Content -LiteralPath $Report
