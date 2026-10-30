# craft-print-diagnostics

Opt-in print observations with a bounded XML validator. No printer APIs, global driver changes, socket
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

## Samsung DarkenText A/B experiment

Default **off**. `PDFCRAFT_PRINT_AB_DARKEN_TEXT=Minimum` requests the unchanged
Minimum baseline; `Off` requests the advertised Samsung Off option **only on the
job ticket**. Missing/empty disables this experiment. Both modes require the
absolute diagnostic sink setting above. Invalid settings refuse the experiment.
This is not a UI/default preference and is not saved to the Windows queue.

After an approved build, close existing PdfCraft instances. From the checkout's
`packaging/windows` directory, run each arm separately, with the same executable:

```powershell
.\Run-SamsungDarkenTextAB.ps1 -Mode Minimum -Executable 'C:\PdfCraft\pdfcraft.exe'
.\Run-SamsungDarkenTextAB.ps1 -Mode Off -Executable 'C:\PdfCraft\pdfcraft.exe'
```

Each launcher prints its new `%TEMP%\pdfcraft-darken-<Mode>-<time>-<GUID>.txt`
path, starts the app, waits for it to close and restores the calling process's
previous environment. It neither prints automatically nor changes printer
preferences. Older executables do not implement this experiment.

For **each arm**, select the same PDF and Samsung queue, A4, **Actual**, black/white,
same orientation, page selection, copies and content. Make one submission then
close the app. Do not use the driver-preferences button: it can save defaults.
The code refuses Fit/custom scaling, non-A4, non-600 DPI, color, FineEdge other
than Normal, unsupported/ambiguous options or a baseline that is not Minimum.
It never substitutes DPI. Explicit image printing and a compatibility image
fallback are refused for this experiment; normal mode retains those paths.

The baseline ticket is copied and normalized in a read-only probe. Off uses the
capability's actual numeric option value and property kind; only DarkenText is
replaced. All other semantic ticket fields are compared after validation. The
actual normalized DEVMODE is converted back to a ticket, then checked again
against the baseline (including public DEVMODE bytes). Missing/reverted settings,
normalization drift, a non-600 actual rasterDPI or an unavailable required audit
sink cause refusal before the native print target is created. No `DM_UPDATE`,
queue Commit, Set-Printer*, default-printer change or policy write is introduced.
Unknown/volatile driver fields can conservatively prevent the experiment.

Use the two resulting logs with the read-only comparator:

```powershell
.\Compare-SamsungDarkenTextAB.ps1 -MinimumLog 'C:\logs\A.txt' -OffLog 'C:\logs\B.txt'
```

It requires one completed submission per log, the same input fingerprint and
vector route, Actual 100%, actual rasterDPI 600, and equal protected-ticket/public
DEVMODE fingerprints. `SETTINGS_MATCH_ONLY` is **not proof of physical quality or
of printer-firmware behavior**. Fingerprints are correlation checks, not security
digests. If comparison fails, do not treat the printouts as a one-variable test.

`ab.validation` shows requested/returned selection and rejection; `ab.normalized`
shows the post-DocumentPropertiesW selection and public DPI/scale; `ab.rasterDPI`
and `native.CreatePrintControl` show the actual unchanged calculation/call value.
Successful confirmation is labelled `TICKET_ONLY`, physical effect `UNVERIFIED`.
Raw tickets, private driver bytes, PDF text, document names and arbitrary vendor
values are not written. The baseline's protected settings are compared in memory.

Compare the two physical labels at the same size: small character counters/gaps,
1D bar/space separation, QR module boundaries, and repeated barcode/QR scans under
the same conditions. Mark A/B on the back after printing; no label/PDF content is
modified by this feature. No automatic thickness/scan-quality measurement is claimed.

Host regressions cover accepted/reverted/unknown tickets, other-option drift,
DPI/scale rejection, namespace/parameter handling, log privacy, normal/Type 1C byte
preservation and image-fallback refusal only during the experiment. The standalone
PowerShell fixture test uses synthetic logs and never launches a GUI or printer.
Windows provider execution and company-printer output require separate validation.
