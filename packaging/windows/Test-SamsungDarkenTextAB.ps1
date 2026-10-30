# Synthetic metadata-only fixtures. Never launches PdfCraft or uses any printer API.
[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Checks=0
function Check($Value,[string]$Name) { if (!$Value) { throw ('Fixture failed: '+$Name) }; $script:Checks++ }
function Must-Fail([scriptblock]$Action,[string]$Name) { $Failed=$false; try { & $Action | Out-Null } catch { $Failed=$true }; Check $Failed $Name }
foreach ($Name in @('Run-SamsungDarkenTextAB.ps1','Compare-SamsungDarkenTextAB.ps1')) {
  $Tokens=$null; $Errors=$null
  $Ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $Name),[ref]$Tokens,[ref]$Errors)
  Check (!$Errors.Count) ($Name+' syntax')
  foreach ($Command in $Ast.FindAll({param($N) $N -is [Management.Automation.Language.CommandAst]},$true)) {
    Check ($Command.GetCommandName() -notin @('Set-Printer','Set-PrintConfiguration','Set-PrinterProperty','Add-Printer','Remove-Printer','Out-Printer','Invoke-WebRequest','Invoke-RestMethod','Set-ItemProperty','Set-ExecutionPolicy')) ($Name+' has no printer/policy/network mutation')
  }
}
. (Join-Path $PSScriptRoot 'Compare-SamsungDarkenTextAB.ps1') -LibraryOnly
function Fixture([string]$Mode,[string]$Job) {
  $Prefix='unix_ms=100 pid=1 input=0123456789abcdef job='+$Job+' stage='
  $Value=if ($Mode -eq 'Off') { '0' } else { '3' }
  return (@(
    ($Prefix+'ab.request requested='+$Mode+' queue_defaults_changed=false'),
    ($Prefix+'ab.validation requested='+$Mode+' observed=Some(('+ $Mode+', '+$Value+')) confirmed_ticket=true status=TICKET_ONLY physical_effect=UNVERIFIED'),
    ($Prefix+'ab.baseline_probe darken=Minimum protected_ticket_id=1111111111111111 public_devmode_id=2222222222222222 actual_job=false dpi=600'),
    ($Prefix+'ab.normalized requested='+$Mode+' observed=Some(('+ $Mode+', '+$Value+')) normalized_public_devmode=Some(...) confirmed_ticket=true status=TICKET_ONLY observed_dpi_x=Some(600) observed_dpi_y=Some(600) observed_scaling_none=Some(true) observed_media_width=Some(210000) observed_media_height=Some(297000) observed_fine_edge_normal=Some(true) physical_effect=UNVERIFIED'),
    ($Prefix+'ab.rasterDPI actual_argument=600 required=600'),
    ($Prefix+'native.CreatePrintControl rasterDPI=600 fontSubset=DEFAULT colorSpace=CUSTOM'),
    ($Prefix+'layout.placement serialized_scale_x=Some(1.0) serialized_scale_y=Some(1.0) scope=PDF_content_not_physical_output'),
    ($Prefix+'print.compatibility path=type1c_vector type1c_conversion=true rasterized_entire_pdf=false'),
    ($Prefix+'print.end success=true')
  ) -join "`n")
}
$Temp=Join-Path ([IO.Path]::GetTempPath()) ('pdfcraft-ab-tests-'+[Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($Temp)
try {
  $APath=Join-Path $Temp 'A.txt'; $BPath=Join-Path $Temp 'B.txt'
  [IO.File]::WriteAllText($APath,(Fixture 'Minimum' '1-1-1')); [IO.File]::WriteAllText($BPath,(Fixture 'Off' '1-1-2'))
  $A=Read-AbEvidence $APath 'Minimum'; $B=Read-AbEvidence $BPath 'Off'
  Check ((Compare-AbEvidence $A $B).StartsWith('SETTINGS_MATCH_ONLY')) 'matching evidence remains limited to settings'
  $Hash=(Get-FileHash -LiteralPath $APath).Hash
  & (Join-Path $PSScriptRoot 'Compare-SamsungDarkenTextAB.ps1') -MinimumLog $APath -OffLog $BPath | Out-Null
  Check ((Get-FileHash -LiteralPath $APath).Hash -eq $Hash) 'input unchanged'
  $Base=Fixture 'Off' '1-1-2'
  foreach ($Bad in @(
    $Base.Replace('confirmed_ticket=true','confirmed_ticket=false'),
    $Base.Replace('rasterDPI=600','rasterDPI=1200'),
    $Base.Replace('observed_dpi_x=Some(600)','observed_dpi_x=Some(1200)'),
    $Base.Replace('Some(1.0)','Some(0.94)'),
    $Base.Replace('success=true','success=false'),
    $Base.Replace('path=type1c_vector type1c_conversion=true','path=image_fallback type1c_conversion=false'),
    ($Base+"`n"+$Base)
  )) {
    [IO.File]::WriteAllText($BPath,$Bad)
    Must-Fail { Read-AbEvidence $BPath 'Off' } 'incomplete, inconsistent or multiple-job evidence refused'
  }
  [IO.File]::WriteAllText($BPath,$Base.Replace('1111111111111111','3333333333333333'))
  $Mismatch=Read-AbEvidence $BPath 'Off'
  Must-Fail { Compare-AbEvidence $A $Mismatch } 'other-option difference refused'
} finally { [IO.Directory]::Delete($Temp,$true) }
Write-Output "Samsung A/B offline fixtures passed: $Checks checks. No printer/GUI was used."
