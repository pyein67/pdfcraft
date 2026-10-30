# Read-only comparison of two complete single-job logs; does not inspect PDFs or printers.
[CmdletBinding()]
param([string]$MinimumLog,[string]$OffLog,[switch]$LibraryOnly)
$ErrorActionPreference='Stop'
function Read-AbEvidence([string]$Path,[string]$ExpectedMode) {
  $File=Get-Item -LiteralPath $Path
  if ($File.Length -gt 16777216) { throw 'A/B log exceeds 16 MiB.' }
  $Stream=[IO.File]::OpenRead($File.FullName)
  try {
    $Memory=[IO.MemoryStream]::new()
    try {
      $Buffer=New-Object byte[] 8192
      while (($Read=$Stream.Read($Buffer,0,$Buffer.Length)) -gt 0) {
        if ($Memory.Length+$Read -gt 16777216) { throw 'A/B log grew beyond size limit.' }
        $Memory.Write($Buffer,0,$Read)
      }
      $Text=[Text.Encoding]::UTF8.GetString($Memory.ToArray())
    } finally { $Memory.Dispose() }
  } finally { $Stream.Dispose() }
  $Entries=[Collections.Generic.List[object]]::new()
  foreach ($Line in ($Text -split '\r?\n')) {
    if ($Line -match '^unix_ms=\d+ pid=\d+ input=([0-9a-f]{16}) job=([0-9-]{1,96}) stage=([A-Za-z0-9_.]+) (.*)$') {
      if ($Entries.Count -ge 50000) { throw 'A/B log entry limit exceeded.' }
      $Entries.Add([pscustomobject]@{ Input=$Matches[1]; Job=$Matches[2]; Stage=$Matches[3]; Data=$Matches[4] })
    }
  }
  $Requests=@($Entries | Where-Object Stage -eq 'ab.request')
  if ($Requests.Count -ne 1 -or $Requests[0].Data -notmatch ('^requested='+$ExpectedMode+' ')) { throw 'Require exactly one A/B job of the expected mode per log.' }
  $Job=$Requests[0]
  $Same=@($Entries | Where-Object { $_.Job -eq $Job.Job -and $_.Input -eq $Job.Input })
  if (@($Same | Where-Object { $_.Stage -eq 'print.end' -and $_.Data -eq 'success=true' }).Count -ne 1) { throw 'A/B submission success is unconfirmed.' }
  $SelectedPattern='^requested='+$ExpectedMode+' observed=Some\(\('+$ExpectedMode+', -?\d+\)\) '
  $Validation=@($Same | Where-Object { $_.Stage -eq 'ab.validation' -and $_.Data -match $SelectedPattern -and $_.Data -match 'confirmed_ticket=true status=TICKET_ONLY' })
  $Normalized=@($Same | Where-Object { $_.Stage -eq 'ab.normalized' -and $_.Data -match $SelectedPattern -and $_.Data -match 'confirmed_ticket=true status=TICKET_ONLY' -and $_.Data -match 'observed_dpi_x=Some\(600\) observed_dpi_y=Some\(600\) observed_scaling_none=Some\(true\) observed_media_width=Some\(210000\) observed_media_height=Some\(297000\) observed_fine_edge_normal=Some\(true\)' })
  $Raster=@($Same | Where-Object { $_.Stage -eq 'ab.rasterDPI' -and $_.Data -eq 'actual_argument=600 required=600' })
  $Control=@($Same | Where-Object { $_.Stage -eq 'native.CreatePrintControl' -and $_.Data -match '^rasterDPI=600 ' })
  if ($Validation.Count -ne 1 -or $Normalized.Count -ne 1 -or $Raster.Count -ne 1 -or $Control.Count -ne 1) { throw 'Complete validated/normalized ticket and actual 600 DPI evidence is required.' }
  $Placement=@($Entries | Where-Object { $_.Input -eq $Job.Input -and $_.Stage -eq 'layout.placement' })
  if (!$Placement.Count -or @($Placement | Where-Object { $_.Data -notmatch 'serialized_scale_x=Some\(1(?:\.0)?\) serialized_scale_y=Some\(1(?:\.0)?\)' }).Count) { throw 'Actual 100% layout evidence is missing or inconsistent.' }
  $Probe=@($Same | Where-Object Stage -eq 'ab.baseline_probe')
  if ($Probe.Count -ne 1 -or $Probe[0].Data -notmatch 'protected_ticket_id=([0-9a-f]{16}) public_devmode_id=([0-9a-f]{16}) ') { throw 'Protected baseline evidence unavailable.' }
  $Ticket=$Matches[1]; $DevMode=$Matches[2]
  $Compatibility=@($Same | Where-Object { $_.Stage -eq 'print.compatibility' -and $_.Data -match '^path=type1c_vector type1c_conversion=true ' })
  $Normal=@($Same | Where-Object { $_.Stage -eq 'print.route' -and $_.Data -eq 'path=normal_native type1c_conversion=false' })
  $Route=if ($Compatibility.Count -eq 1) { 'type1c_vector' } elseif ($Normal.Count -eq 1) { 'normal_native' } else { throw 'Same vector route cannot be confirmed.' }
  return [pscustomobject]@{ Mode=$ExpectedMode; Input=$Job.Input; ProtectedTicket=$Ticket; PublicDevMode=$DevMode; Route=$Route }
}
function Compare-AbEvidence($A,$B) {
  foreach ($Name in @('Input','ProtectedTicket','PublicDevMode','Route')) {
    if ($A.$Name -ne $B.$Name) { throw ('A/B mismatch: '+$Name+'. Do not treat this as a single-variable comparison.') }
  }
  return 'SETTINGS_MATCH_ONLY: Minimum / Off tickets confirmed; same PDF fingerprint, vector route, protected ticket and public DEVMODE; Actual 100%, 600 DPI. Physical darkening/quality effect remains UNVERIFIED.'
}
if ($LibraryOnly) { return }
$A=Read-AbEvidence $MinimumLog 'Minimum'
$B=Read-AbEvidence $OffLog 'Off'
Compare-AbEvidence $A $B
