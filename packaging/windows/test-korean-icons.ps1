# Native verification of the actual PE resource images and isolated PDF registration.
param([Parameter(Mandatory)][string]$Executable,
      [string]$Icon = (Join-Path $PSScriptRoot '../../assets/app-icon/korean/pdfcraft-pdf-icon.ico'))
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path -LiteralPath $Executable).Path
[byte[]]$pe = [IO.File]::ReadAllBytes($Exe)
[byte[]]$ico = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Icon).Path)
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
foreach ($groupId in @(1, 2)) {
  $group = Resource-Bytes 14 $groupId
  $count = U16 $ico 4
  if ((U16 $group 4) -ne $count) { throw "Icon group $groupId has wrong frame count." }
  for ($i = 0; $i -lt $count; $i++) {
    if ((U16 $group (6 + 14 * $i + 4)) -ne 1) { throw "EXE group $groupId frame $i has noncanonical color planes." }
    $entry = 6 + 16 * $i
    $offset = U32 $ico ($entry + 12)
    $size = U32 $ico ($entry + 8)
    $expected = [byte[]]$ico[$offset..($offset + $size - 1)]
    $actual = Resource-Bytes 3 (U16 $group (6 + 14 * $i + 12))
    if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($expected)) {
      throw "EXE resource $groupId frame $i differs from the supplied ICO."
    }
  }
}
Write-Output 'PASS: x64 EXE app and PDF resources contain all 7 original icon frames byte-for-byte.'
Add-Type -AssemblyName System.Drawing
$loadedIcon = [Drawing.Icon]::ExtractAssociatedIcon($Exe)
if (!$loadedIcon) { throw 'Win32 could not load the executable icon.' }
try {
  $bitmap = $loadedIcon.ToBitmap()
  try { if ($bitmap.Width -lt 16 -or $bitmap.Height -lt 16) { throw 'Win32 returned an invalid icon bitmap.' } }
  finally { $bitmap.Dispose() }
} finally { $loadedIcon.Dispose() }
Write-Output 'PASS: the Windows icon loader produced a usable bitmap.'

. (Join-Path $PSScriptRoot 'Register-PdfCraft.ps1')
$testPath = 'Software\PdfCraftIconTests\' + [guid]::NewGuid().ToString()
$root = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($testPath)
try {
  $fakeExe = 'C:\한글 경로\PdfCraft\pdfcraft.exe'
  $pdf = $root.CreateSubKey('Software\Classes\.pdf')
  $pdf.SetValue('', 'Existing.Reader'); $pdf.Dispose()
  Set-PdfCraftRegistration $root $fakeExe
  Set-PdfCraftRegistration $root $fakeExe # Updating an existing registration must also preserve defaults.
  foreach ($keyName in @('PdfCraft.Document', 'Applications\pdfcraft.exe')) {
    $key = $root.OpenSubKey("Software\Classes\$keyName\DefaultIcon")
    try { if ($key.GetValue('') -cne ('"' + $fakeExe + '",-2')) { throw 'Wrong PDF DefaultIcon.' } }
    finally { $key.Dispose() }
    $key = $root.OpenSubKey("Software\Classes\$keyName\shell\open\command")
    try { if ($key.GetValue('') -cne ('"' + $fakeExe + '" "%1"')) { throw 'Broken Unicode/spaced PDF open command.' } }
    finally { $key.Dispose() }
  }
  $pdf = $root.OpenSubKey('Software\Classes\.pdf')
  try { if ($pdf.GetValue('') -cne 'Existing.Reader') { throw 'Existing PDF default was changed.' } }
  finally { $pdf.Dispose() }
  $choice = $root.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.pdf\UserChoice')
  if ($choice) { $choice.Dispose(); throw 'Registration must not create UserChoice.' }
} finally {
  $root.Dispose()
  [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($testPath)
}
Write-Output 'PASS: isolated current-user registration uses correct icon/open commands and preserves the existing default.'
# Launch the real GUI with the same quoted file arguments used by the registration.
$work = Join-Path ([IO.Path]::GetTempPath()) ('PdfCraft 한글 icon ' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory $work | Out-Null
$process = $null
try {
  $pdfPath = Join-Path $work '한글 문서.pdf'
  $text = "%PDF-1.4`n"
  $offsets = @(0)
  foreach ($object in @('<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << >> >>')) {
    $offsets += $text.Length
    $text += "$($offsets.Count - 1) 0 obj`n$object`nendobj`n"
  }
  $xref = $text.Length
  $text += "xref`n0 4`n0000000000 65535 f `n"
  foreach ($offset in $offsets[1..3]) { $text += ('{0:D10} 00000 n ' -f $offset) + "`n" }
  $text += "trailer`n<< /Size 4 /Root 1 0 R >>`nstartxref`n$xref`n%%EOF`n"
  [IO.File]::WriteAllText($pdfPath, $text, [Text.Encoding]::ASCII)
  $control = Join-Path $work 'control.json'
  $cli = Join-Path (Split-Path $Exe -Parent) 'pdfcraft-cli.exe'
  $process = Start-Process -FilePath $Exe -ArgumentList @(('"' + $pdfPath + '"'), '--new-window', '--control', ('"' + $control + '"')) -PassThru
  $opened = $false
  for ($i = 0; $i -lt 60; $i++) {
    if ($process.HasExited) { throw "GUI exited before opening the PDF (exit $($process.ExitCode))." }
    if (Test-Path -LiteralPath $control) {
      $stateJson = & $cli ui --control $control state
      if ($LASTEXITCODE -eq 0) {
        $state = ($stateJson -join "`n") | ConvertFrom-Json
        if (@($state.documents | Where-Object { $_.path -eq $pdfPath -and $_.pages -eq 1 }).Count -eq 1) { $opened = $true; break }
      }
    }
    Start-Sleep -Seconds 1
  }
  if (!$opened) { throw 'GUI did not open the Unicode/spaced PDF path within 60 seconds.' }
  & $cli ui --control $control quit
  if ($LASTEXITCODE -ne 0) { throw 'GUI normal quit failed.' }
  if (!$process.WaitForExit(15000)) { throw 'GUI did not exit after normal quit.' }
  if ($process.ExitCode -ne 0) { throw "GUI exit code $($process.ExitCode)." }
  Write-Output 'PASS: real Windows GUI launched, opened a Unicode/spaced PDF path and exited through the opt-in control channel.'
} finally {
  if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force }
  Remove-Item -LiteralPath $work -Recurse -Force
}
