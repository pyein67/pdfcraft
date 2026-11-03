<# Read-only print diagnostics. Reports may contain printer/host/document names: review before sharing.
   Does not change printer defaults, clear queues, enable logging or enable crash dumps. #>
[CmdletBinding()]
param([int]$Days = 3, [string]$OutputDirectory = (Join-Path ([Environment]::GetFolderPath('Desktop')) ('PdfCraft-print-diagnostics-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))))
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$Since = (Get-Date).AddDays(-[Math]::Min(30, [Math]::Max(1, $Days)))
Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, OSArchitecture | Format-List | Out-File (Join-Path $OutputDirectory 'windows.txt') -Encoding utf8
Get-Printer | Select-Object Name, DriverName, PortName, PrinterStatus, Type | Format-List | Out-File (Join-Path $OutputDirectory 'printers.txt') -Encoding utf8
Get-PrinterDriver | Select-Object Name, Manufacturer, MajorVersion, DriverVersion, InfPath | Format-List | Out-File (Join-Path $OutputDirectory 'drivers.txt') -Encoding utf8
foreach ($Log in @('Application', 'Microsoft-Windows-PrintService/Admin', 'Microsoft-Windows-PrintService/Operational')) {
  $Name = $Log.Replace('/', '-')
  try {
    $Filter = @{ LogName = $Log; StartTime = $Since }
    if ($Log -eq 'Application') { $Filter.Id = @(1000, 1001, 1002) }
    $Events = Get-WinEvent -FilterHashtable $Filter -MaxEvents 400 -ErrorAction Stop
    if ($Log -eq 'Application') { $Events = $Events | Where-Object { $_.Id -in @(1000, 1001, 1002) -and $_.Message -match 'pdfcraft|PrintIsolationHost|spoolsv' } }
    $Events | Select-Object TimeCreated, Id, ProviderName, LevelDisplayName, Message | Format-List | Out-File (Join-Path $OutputDirectory "$Name.txt") -Encoding utf8
  } catch { $_.Exception.Message | Out-File (Join-Path $OutputDirectory "$Name-unavailable.txt") -Encoding utf8 }
}
Write-Output "Diagnostics saved to $OutputDirectory. Review printer/host/document names before sharing."
Write-Output 'Also keep PdfCraft log files and the time of the failure. Crash dump setup is documented in WINDOWS-PRINT-FIX.md.'
