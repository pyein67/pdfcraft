# Process-local experiment selector. No printer APIs or automatic printing.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][ValidateSet('Minimum','Off')][string]$Mode,
      [Parameter(Mandatory=$true)][string]$Executable,
      [string]$LogDirectory=[IO.Path]::GetTempPath())
$ErrorActionPreference='Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Windows에서 실행하세요.' }
$Exe=Get-Item -LiteralPath $Executable
if ($Exe.PSIsContainer -or $Exe.Name -ine 'pdfcraft.exe') { throw '테스트 버전의 pdfcraft.exe를 지정하세요.' }
$Directory=Get-Item -LiteralPath $LogDirectory
if (!$Directory.PSIsContainer) { throw '존재하는 로그 디렉터리를 지정하세요.' }
$Log=Join-Path $Directory.FullName ('pdfcraft-darken-'+$Mode+'-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+[Guid]::NewGuid().ToString('N')+'.txt')
$OldLog=[Environment]::GetEnvironmentVariable('PDFCRAFT_PRINT_DIAGNOSTICS','Process')
$OldMode=[Environment]::GetEnvironmentVariable('PDFCRAFT_PRINT_AB_DARKEN_TEXT','Process')
try {
  $env:PDFCRAFT_PRINT_DIAGNOSTICS=$Log
  $env:PDFCRAFT_PRINT_AB_DARKEN_TEXT=$Mode
  Write-Host ('A/B 모드: '+$Mode+' / 로그: '+$Log)
  Write-Host '동일 PDF / 동일 삼성 프린터 / A4 / Actual 100% / 흑백을 선택하세요.'
  Write-Host 'DPI 600과 FineEdge Normal 등 다른 설정은 유지해야 합니다. 조건이 다르면 작업을 거부합니다.'
  Write-Host '이 스크립트는 직접 인쇄하거나 프린터 기본값을 변경하지 않습니다. 앱에서 인쇄한 뒤 종료하세요.'
  Start-Process -FilePath $Exe.FullName -WorkingDirectory $Exe.DirectoryName -Wait
} finally {
  [Environment]::SetEnvironmentVariable('PDFCRAFT_PRINT_DIAGNOSTICS',$OldLog,'Process')
  [Environment]::SetEnvironmentVariable('PDFCRAFT_PRINT_AB_DARKEN_TEXT',$OldMode,'Process')
}
Write-Output $Log
