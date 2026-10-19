# CI-only native API probe. Uses a temporary file-port queue, never a physical printer/default.
param([Parameter(Mandatory)][string]$Cli, [Parameter(Mandatory)][string]$ReportDirectory)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $ReportDirectory | Out-Null
$ReportDirectory = (Resolve-Path $ReportDirectory).Path
$driver = Get-PrinterDriver -Name 'Microsoft Print To PDF' -ErrorAction SilentlyContinue
if (!$driver) {
  try { Add-PrinterDriver -Name 'Microsoft Print To PDF'; $driver = Get-PrinterDriver -Name 'Microsoft Print To PDF' }
  catch {
    @{ Status = 'NOT_RUN'; Reason = 'Microsoft Print to PDF driver unavailable on the CI host'; Detail = $_.Exception.Message } | ConvertTo-Json | Set-Content (Join-Path $ReportDirectory 'print-to-pdf-result.json') -Encoding utf8
    Write-Output '::warning title=Microsoft Print to PDF::NOT RUN: driver unavailable on the CI host.'
    return
  }
}
$cases = @(@('english',1,'English printing test'), @('korean',1,'한글 인쇄 시험 ABC 123'), @('multipage',3,'한글 여러 페이지 English 123'))
$results = @()
foreach ($case in $cases) {
  $name = 'PdfCraft PDF regression ' + [guid]::NewGuid().ToString()
  $output = Join-Path $ReportDirectory ($case[0] + '-printed.pdf')
  try {
    Add-PrinterPort -Name $output
    Add-Printer -Name $name -DriverName $driver.Name -PortName $output
    $steps = @(@{ tool = 'doc_create'; args = @{ from = 'blank'; pages = $case[1]; width = 612; height = 792 } })
    for ($page = 1; $page -le $case[1]; $page++) {
      $steps += @{ tool = 'page_add_text'; args = @{ doc = 1; page = $page; text = $case[2]; rect = @(40,40,500,100); size = 14 } }
    }
    $steps += @{ tool = 'doc_print'; args = @{ doc = 1; printer = $name; paper = 'Letter'; layout = 'actual' } }
    $script = Join-Path $ReportDirectory ($case[0] + '.json')
    ConvertTo-Json -InputObject $steps -Depth 8 | Set-Content -LiteralPath $script -Encoding utf8
    # Bound the native process so a blocked print driver cannot hang the build indefinitely.
    $stdout = Join-Path $ReportDirectory ($case[0] + '-stdout.txt')
    $stderr = Join-Path $ReportDirectory ($case[0] + '-stderr.txt')
    $process = Start-Process -FilePath $Cli -ArgumentList @('run','--script',('"' + $script + '"'),'--root',('"' + $ReportDirectory + '"')) -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    if (!$process.WaitForExit(180000)) { Stop-Process -Id $process.Id -Force; throw 'Print to PDF native call timed out (3 minutes).' }
    if ($process.ExitCode -ne 0) { throw "Print to PDF exit $($process.ExitCode): $(Get-Content $stderr -Raw)" }
    $until = (Get-Date).AddSeconds(30)
    while (!(Test-Path -LiteralPath $output) -and (Get-Date) -lt $until) { Start-Sleep -Milliseconds 100 }
    $opened = & $Cli run doc_open "path=$output" --root $ReportDirectory | ConvertFrom-Json
    if ($LASTEXITCODE -or $opened.pages -ne $case[1]) { throw 'Printed PDF could not be reopened with the expected page count.' }
    $results += @{ Case = $case[0]; Status = 'OUTPUT_VERIFIED'; Pages = $opened.pages }
  } catch {
    $results += @{ Case = $case[0]; Status = 'FAILED'; Error = $_.Exception.Message }
  } finally {
    Get-PrintJob -PrinterName $name -ErrorAction SilentlyContinue | Remove-PrintJob -ErrorAction SilentlyContinue
    Remove-Printer -Name $name -ErrorAction SilentlyContinue
  }
}
$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $ReportDirectory 'print-to-pdf-result.json') -Encoding utf8
$failed = @($results | Where-Object Status -eq 'FAILED')
if ($failed.Count) { throw ($failed | ConvertTo-Json -Depth 5) }
Write-Output 'PASS: actual Microsoft Print to PDF output reopened for English, Korean and three-page cases. No physical printer was tested.'
