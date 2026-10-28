# Conditional image printing and Windows crash containment

Updated: 2026-10-28. Candidate on `fix/pdf-type3-print-crash`, based exactly on
`999126c88c4287117ee25b39e442c1e33c4855c1`. Native Windows/hardware acceptance is pending.

## Evidence and scope

The privately supplied regression PDF has one A4 page and an embedded Type 3 font.
The user reports successful preview, a silent exit on original native printing, and
successful printing after conversion to a 600 dpi image PDF. This suggests avoiding
original PDF constructs at the native rendering boundary; it does not prove a font,
Windows component or printer driver caused the fault. No faulting stack is available.
The private file, its content and derived images are not part of the repository or CI.

The existing path is permission checking → imposition of current document/annotations →
`craft-print` → Windows.Data.Pdf/Direct2D/XPS plus the driver's validated PrintTicket.
These native calls occur inside the caller; Rust Result/catch_unwind cannot contain an
access violation. The pinned backend and dependency versions have not been changed.

## Behavior

* Ordinary Windows PDFs without Type 3 page-resource fonts, with image printing disabled,
  retain the exact imposed bytes and old in-process vector submission call.
* Effective page resources are inspected structurally, including inheritance, indirect
  objects, compressed object streams and nested Form XObjects. Cycles are tracked and
  graph work/depth bounded. An unused Type 3 resource can conservatively select the route;
  this is not a claim that every Type 3 document is defective.
* Type 3 jobs, or explicit image printing, launch the **same** application executable
  using an internal worker flag before GUI/single-instance/MCP initialization. No shell,
  additional application, network listener or installed service is used. Both desktop and
  CLI recognize the worker protocol. This isolates only compatibility jobs.
* The worker renders the final imposed sheets with PdfCraft's existing renderer, at
  600 dpi, in 2048px lossless RGB tiles. No original fonts, vectors or CharProcs are sent
  into Windows.Data.Pdf. Physical sheet dimensions, order, rotation and imposition are
  retained. The complete job is prepared before any spool submission.
* Printer, copies, collate, duplex (including driver default), grayscale, scaling and
  options are carried without changing the backend's ticket handling. Source documents
  and their searchable/editable Korean text are untouched. Save as PDF stays vector unless
  `doc_print` explicitly requests image output. The original icons and registration stay.
* The desktop receives a normal error if the worker crashes, fails, gives an invalid
  response or exceeds 15 minutes. A killed/failed job is **never retried automatically**:
  inspect the printer queue first because some pages may already have printed.

## Limits

RGB rasterization flattens text and transparency; spot/CMYK output, selectable text in the
print-only PDF and vector sharpness beyond 600 dpi are not retained. Raster output can be
larger/slower. This is compatibility printing, not a general archival PDF conversion.
Raster quality/barcode readability, physical margins and colours require actual printing.
Windows native discovery/preferences remain the existing implementation; unrelated driver
crashes during those operations are outside the isolated job boundary.

Input/output limit: 256 MiB. Pixel limits: 100 million per sheet, 500 million per job;
at most 1000 sheets. Resource scan: 100000 nodes, depth 128. These are refusals with an
error, not silent resolution reduction. Renderer errors or content-truncation warnings
abort before submission. Tiles limit the output pixel buffer size; source decoding and
renderer internals retain their existing allocation limits. Temporary inputs/results are
removed after the worker exits; abnormal parent/OS termination can leave temporary files.

## Validation and Windows build

Host tests cover nested/inherited Type 3 detection, compressed dictionaries, resource
cycles, raw-string false positives, raster dimensions/ink, rotation/order/tile edges,
budget rejection, worker option roundtrip, file bounds, exit failure and timeout cleanup.
The image-removal regression was observed failing with the old pass-through behavior.
Automation and real CLI tests exercise vector vs image PDF output and confinement.
`private_print_pdf_is_detected_and_flattens_without_fonts` is ignored by default; run it
locally with `PDFCRAFT_PRIVATE_PRINT_PDF` set to a private repro path. Never commit that PDF.

The existing `packaging/windows/build-korean.ps1` now includes image/worker and CLI
regressions alongside the existing Korean/font/automation/UI and icon checks. The dedicated
Windows workflow accepts the new branch and retains native spool/virtual-PDF probes.
The current-binary virtual-PDF probe adds explicit isolated 600 dpi image output;
the original comparison binaries retain their old arguments and three vector cases.
Its old-branch release publishing condition is retained: a new branch does not overwrite
the original release or publish a preview automatically.

On Windows, use Visual Studio 2022 Developer PowerShell with Desktop development with C++
and the Windows SDK installed:

```powershell
.\packaging\windows\build-korean.ps1
```

The external verified craft-fonts input is retained; do not omit the Korean build input.
Output: `target/x86_64-pc-windows-msvc/release/pdfcraft.exe` and `pdfcraft-cli.exe`, plus
existing required font/asset licences and registration files. No extra helper EXE is needed.
Do not use `-SkipTests` for the acceptance build.

Native acceptance: compare ordinary English/Korean/multipage PDFs and the private repro
on the actual Windows queue; confirm paper output, barcode scanning and retained UI/icons.
Also test explicit image printing, cancellation and failed submission. A successful
host rasterization or Microsoft Print to PDF result does not establish physical output.

History: 2026-10-28 — conditional 600 dpi image route and same-executable worker candidate;
original native fault cause and physical printer acceptance remain unverified.
