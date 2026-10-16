<# Register this ZIP's PdfCraft for Open with / Default apps, for the current user only.
   Existing PDF defaults are preserved. Choose PdfCraft in Windows Settings to use its PDF icon.
   Existing registration keys are exported before changes; keep the backup to restore them. #>
[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot 'pdfcraft.exe'),
      [string]$BackupDirectory = (Join-Path $PSScriptRoot 'registration-backup'))
$ErrorActionPreference = 'Stop'

function Set-PdfCraftRegistration([Microsoft.Win32.RegistryKey]$Root, [string]$Exe) {
  # The root is HKCU in normal use; tests supply a temporary, isolated registry subtree.
  $values = @(
    @('Software\Classes\PdfCraft.Document', '', 'PDF Document'),
    @('Software\Classes\PdfCraft.Document\DefaultIcon', '', ('"' + $Exe + '",-2')),
    @('Software\Classes\PdfCraft.Document\shell\open\command', '', ('"' + $Exe + '" "%1"')),
    @('Software\Classes\.pdf\OpenWithProgids', 'PdfCraft.Document', ''),
    @('Software\Classes\Applications\pdfcraft.exe', 'FriendlyAppName', 'PdfCraft'),
    @('Software\Classes\Applications\pdfcraft.exe\DefaultIcon', '', ('"' + $Exe + '",-2')),
    @('Software\Classes\Applications\pdfcraft.exe\SupportedTypes', '.pdf', ''),
    @('Software\Classes\Applications\pdfcraft.exe\shell\open\command', '', ('"' + $Exe + '" "%1"')),
    @('Software\PdfCraft\Capabilities', 'ApplicationName', 'PdfCraft'),
    @('Software\PdfCraft\Capabilities', 'ApplicationDescription', 'Open-source PDF workbench'),
    @('Software\PdfCraft\Capabilities', 'ApplicationIcon', ('"' + $Exe + '",-1')),
    @('Software\PdfCraft\Capabilities\FileAssociations', '.pdf', 'PdfCraft.Document'),
    @('Software\RegisteredApplications', 'PdfCraft', 'Software\PdfCraft\Capabilities')
  )
  foreach ($entry in $values) {
    $key = $Root.CreateSubKey($entry[0])
    try { $key.SetValue($entry[1], $entry[2], [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
  }
}

if ($MyInvocation.InvocationName -ne '.') {
  $Exe = (Resolve-Path -LiteralPath $Executable).Path
  if ([IO.Path]::GetFileName($Exe) -ine 'pdfcraft.exe') { throw 'Executable must be pdfcraft.exe.' }
  # Save each existing subtree separately. Never overwrite a previous backup.
  $Backup = Join-Path $BackupDirectory ([guid]::NewGuid().ToString())
  New-Item -ItemType Directory -Path $Backup -Force | Out-Null
  $keys = @('Software\Classes\PdfCraft.Document', 'Software\Classes\Applications\pdfcraft.exe',
            'Software\Classes\.pdf\OpenWithProgids', 'Software\PdfCraft\Capabilities',
            'Software\RegisteredApplications')
  for ($i = 0; $i -lt $keys.Count; $i++) {
    if (Test-Path -LiteralPath ('Registry::HKEY_CURRENT_USER\' + $keys[$i])) {
      & reg.exe export ('HKCU\' + $keys[$i]) (Join-Path $Backup "$i.reg") /y | Out-Null
      if ($LASTEXITCODE -ne 0) { throw "Cannot back up $($keys[$i]); registration was not changed." }
    }
  }
  $keys | Set-Content -LiteralPath (Join-Path $Backup 'keys.txt')
  Set-PdfCraftRegistration ([Microsoft.Win32.Registry]::CurrentUser) $Exe
  Write-Output "Registered $Exe. Backup: $Backup"
  Write-Output 'Choose PdfCraft for .pdf in Windows Settings > Apps > Default apps. Existing defaults were preserved.'
  Write-Output 'Windows may need Explorer to restart/sign out before cached icons refresh.'
}
