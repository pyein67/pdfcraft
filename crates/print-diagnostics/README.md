# craft-print-diagnostics

Std-only, opt-in print observations. No printer APIs, global driver changes, socket
or helper process. Used by PdfCraft, the vendored craft-print spooler and winprint.

Before launching the **diagnostic version**, choose an absolute file in an existing
writable directory, in the same PowerShell session:

```powershell
$env:PDFCRAFT_PRINT_DIAGNOSTICS = Join-Path $env:TEMP 'pdfcraft-native-print.txt'
& .\pdfcraft.exe
Remove-Item Env:PDFCRAFT_PRINT_DIAGNOSTICS
```

Unset/empty/relative settings disable diagnostics. The environment is inherited
by the existing crash-isolation worker. This does not enable the feature in an
older executable. It does not change a queue default, DPI, layout or font policy.
Actual printing occurs only when the user normally requests it.

Records include Unix milliseconds, PID, input correlation fingerprint (not a
security hash) and a unique attempt/job ID. `layout.*` uses the imposed PDF's input
fingerprint to join preparation with submission; preparation and submission have
different attempt IDs. The worker inherits submission's ID, as does the spool
thread. Repeated prints of the same PDF have different submission IDs.

- `layout.placement`: planned scale, **serialized four-decimal cm** and scale,
  normalization matrix; these are PDF coordinates, not physical measurements.
- `print.route`, `print.worker`, `print.compatibility`: original native / Type 1C /
  image fallback or explicit image route; conversion recorded after it succeeds.
- `native.ticket.*`: exact validated ticket sent to WinPdfPrinter, selected DPI,
  scaling, paper, font/edge and color features. Missing fields remain unknown.
- `native.sheet`: actual backend reframe matrix, including identity for Exact.
- `native.devmode`: **after** the existing DocumentPropertiesW normalization,
  public flagged DPI and dmScale. Negative quality enums are not interpreted as DPI.
- `native.CreatePrintControl`: actual rasterDPI/fontSubset/colorSpace arguments;
  rasterDPI controls fallback commands, not all vectors or physical device DPI.
- `native.renderer`, `native.AddPage`: native renderer route and page DIP/pt size.

No new driver query is made to collect these records. The exact existing buffers,
plans and call arguments are observed. Printer outcomes and PDF bytes are not
changed. Writes are best effort: an unavailable sink/full disk must not fail a
job. Bounded records (16 KiB), per-context event budget (10,000), and a 16 MiB file
size guard prevent routine unbounded logs; simultaneous processes can race at
the file-size guard. Choose a fresh file per test. There is no rotation/deletion.
Logs omit PDF text, titles, paths, raw XML and private DEVMODE bytes; they do reveal
page dimensions, source page indices, correlation fingerprints and job settings.
Review before sharing. Disabled diagnostics do not inspect tickets or hash PDFs.

Host tests exercise the sink, scope/worker correlation, XML/DEVMODE parsing and
byte-for-byte output preservation. They do not substitute for Windows/provider
execution or physical output tests. No Windows executable is built by these tests.
