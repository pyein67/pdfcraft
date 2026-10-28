# CI-only native API probe. Uses a temporary file-port queue, never a physical printer/default.
param([Parameter(Mandatory)][string]$Cli, [Parameter(Mandatory)][string]$ReportDirectory, [string]$Label = 'current')
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
# The original comparison binaries predate print_as_image. Preserve their probes exactly;
# only the current candidate exercises the isolated image path on a real virtual queue.
if ($Label -eq 'current') { $cases += ,@('image',1,'한글 이미지 인쇄 English 123') }
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
    $printArgs = @{ doc = 1; printer = $name; paper = 'Letter'; layout = 'actual' }
    if ($case[0] -eq 'image') { $printArgs.print_as_image = $true }
    $steps += @{ tool = 'doc_print'; args = $printArgs }
    $script = Join-Path $ReportDirectory ($case[0] + '.json')
    ConvertTo-Json -InputObject $steps -Depth 8 | Set-Content -LiteralPath $script -Encoding utf8
    # Bound the native process so a blocked print driver cannot hang the build indefinitely.
    $stdout = Join-Path $ReportDirectory ($case[0] + '-stdout.txt')
    $stderr = Join-Path $ReportDirectory ($case[0] + '-stderr.txt')
    $process = Start-Process -FilePath $Cli -ArgumentList @('run','--script',('"' + $script + '"'),'--root',('"' + $ReportDirectory + '"')) -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    if (!$process.WaitForExit(180000)) { Stop-Process -Id $process.Id -Force; throw 'Print to PDF native call timed out (3 minutes).' }
    if ($process.ExitCode -ne 0) { throw "Print to PDF exit $($process.ExitCode): $(Get-Content $stderr -Raw)" }
    # Submission completion is not driver-file completion. A file can exist while its
    # writer is still producing the cross-reference table. Wait for a real reopen, not existence.
    $until = (Get-Date).AddSeconds(30)
    $attempts = 0
    $opened = $null
    $openExit = $null
    $lastOpen = ''
    $reopenOut = Join-Path $ReportDirectory ($case[0] + '-reopen-stdout.txt')
    $reopenErr = Join-Path $ReportDirectory ($case[0] + '-reopen-stderr.txt')
    do {
      if ((Test-Path -LiteralPath $output) -and (Get-Item -LiteralPath $output).Length -gt 0) {
        $attempts++
        $lastOpen = @(& $Cli run doc_open "path=$output" --root $ReportDirectory 2> $reopenErr) -join "`n"
        $openExit = $LASTEXITCODE
        Set-Content -LiteralPath $reopenOut -Value $lastOpen -Encoding utf8
        if ($openExit -eq 0) {
          try { $opened = ConvertFrom-Json -InputObject $lastOpen } catch { $opened = $null }
          if ($opened -and $opened.pages -eq $case[1]) { break }
        }
      }
      Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $until)
    if (!$opened -or $opened.pages -ne $case[1]) {
      $bytes = if (Test-Path -LiteralPath $output) { (Get-Item -LiteralPath $output).Length } else { 0 }
      $header = ''
      if ($bytes) {
        $stream = [IO.File]::Open($output, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try { $first = [byte[]]::new(16); $count = $stream.Read($first,0,$first.Length); $header = [Convert]::ToHexString($first[0..($count-1)]) } finally { $stream.Dispose() }
      }
      $detail = if (Test-Path $reopenErr) { [IO.File]::ReadAllText($reopenErr) } else { 'No nonempty file appeared' }
      throw "Native PDF reopen failed: label=$Label case=$($case[0]) expected=$($case[1]) observed=$($opened.pages) exit=$openExit attempts=$attempts bytes=$bytes header=$header stderr=$detail"
    }
    $results += @{ Build = $Label; Case = $case[0]; Status = 'OUTPUT_REOPENED'; Pages = $opened.pages; ReopenAttempts = $attempts }
    Write-Output "::notice title=Native PDF output $Label $($case[0])::OUTPUT_REOPENED: $($opened.pages) pages; attempts=$attempts. Printed glyph appearance and physical paper output are unverified."
  } catch {
    $results += @{ Build = $Label; Case = $case[0]; Status = 'FAILED'; Error = $_.Exception.Message }
  } finally {
    Get-PrintJob -PrinterName $name -ErrorAction SilentlyContinue | Remove-PrintJob -ErrorAction SilentlyContinue
    Remove-Printer -Name $name -ErrorAction SilentlyContinue
  }
}
$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $ReportDirectory 'print-to-pdf-result.json') -Encoding utf8
$failed = @($results | Where-Object Status -eq 'FAILED')
if ($failed.Count) { throw ($failed | ConvertTo-Json -Depth 5) }
Write-Output "PASS: actual Microsoft Print to PDF output reopened for all $($cases.Count) $Label cases. No physical printer was tested."
