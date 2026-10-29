# Print quality v2: conversion versus the Windows output path

> Reviewed 2026-10-29. Based on `a06fcfccc4f14c8baf88b12e0765057d3f24d5bd`,
> branch `fix/pdf-print-quality-v2`. Investigation and test changes only;
> no runtime print-quality fix is claimed. No GitHub push or Windows build.

The user reports that Type 1C improved output and retained crash protection,
but fine text, barcode and QR output still look heavier/less clear than Acrobat.
The photo supports investigating this; perspective, exposure and driver settings
prevent using it to measure physical strokes or scale.

## User evidence and unresolved Windows inputs

The user confirms the same company printer/paper and improvement from 600dpi
full-sheet images to Type 1C. The user **tentatively recalls 100% scaling**;
this is not a verified Acrobat/PdfCraft setting or physical measurement. Neither
print.prepare/print.submit.end logs, printer model, driver name, validated job
resolution/quality nor intermediate Windows output is available. These remain
unknown; no Windows version, model, driver or DPI is assumed. Actual is the main
controlled comparison, and Fit remains a separately labelled control.

## Controlled comparison

Exported **paired** PDFs from the same unedited private document, with A4 paper
and identical imposition, first Actual and then Fit. One retains Type 3; the other
uses the existing Type 1C compatibility converter. Each pair is rendered by the
**same executable at the same DPI**, separately with Poppler 26.05.0 and
MuPDF 1.26.11 (PyMuPDF binding 1.26.6). No cross-renderer pixel scores are mixed.
The PDFs, PNGs and private scripts/results remain outside the repository.

ROIs in original displayed-page points, top-left origin: small 7pt address text
`[133,283,270,302]`, heading `[115,87,235,101]`, 1D code
`[35,94,78,318]`, QR `[240,201.5,280,242.4]`. The same imposition transform
and raster grid are applied to both members. Heading/code ROIs contain ink;
these are not empty-area comparisons.

The small-text **coverage** change below is Type 1C minus Type 3, using
sum((255-gray)/255). Negative means less integrated ink in that region.
It is an antialiasing-sensitive proxy, not measured toner or physical stem width.

| Layout | DPI | Poppler text coverage change | MuPDF text coverage change | Heading / barcode / QR pair pixels |
| --- | ---: | ---: | ---: | --- |
| Actual | 300 | -5.669% | -0.161% | Identical in each renderer |
| Actual | 600 | -2.256% | -0.223% | Identical in each renderer |
| Fit | 300 | -5.666% | +0.129% | Identical in each renderer |
| Fit | 600 | -2.627% | -0.238% | Identical in each renderer |

At Actual/600, Poppler text pixels below gray 128 change from 16,175 to 15,943;
mean horizontal black run length changes 5.664 -> 5.547 px, vertical 5.376 ->
5.293 px. MuPDF gives 16,237 -> 15,942, horizontal 5.652 -> 5.549 px,
vertical 5.362 -> 5.293 px. At Fit some thresholded run means increase slightly
while integrated coverage decreases. There is **no consistent expansion of stems**
in these reference renders. Run means are cross-section proxies across complex
letters; they must not be presented as a direct stem-thickness measurement.

For the 1D code, three vertical scanlines each contain 55 black runs. Every
start/end coordinate and white gap is identical within each before/after pair.
For QR, the entire selected raster is identical, so module boundaries and gray
edge values match as well. This does not test physical scanner readability.

## Additional renderer controls (same input, no product setting changes)

Poppler is now pinned explicitly to FreeType=yes, thinlinemode=none, with all four
font/vector AA combinations. Actual/A4 small-text integrated coverage changes:

| Font AA | Vector AA | 300dpi | 600dpi |
| --- | --- | ---: | ---: |
| On | On | -5.669% | -2.256% |
| Off | On | +2.393% | -2.229% |
| On | Off | -41.285% | -26.274% |
| Off | Off | -36.267% | -26.254% |

These controls show that the **representation-dependent rasterization policy** can
change coverage substantially, even with matching outlines. Disabling AA also
changes pixel-grid quantization; these results do not isolate autohinting alone.
The extreme off settings are diagnostic controls, not recommended printer settings.
Heading/barcode/QR pair pixels remain identical in every combination and DPI.

A third independent renderer, Ghostscript 10.05.1, was run only as an external
oracle, at the same Actual/A4 geometry and 300/600dpi for both members:

| Text/graphics AlphaBits | 300dpi text coverage change | 600dpi text coverage change |
| ---: | ---: | ---: |
| 1/1 | -35.891% | -25.328% |
| 4/4 | -14.558% | -8.413% |

Again, heading/barcode/QR pixels match within each pair. None of these measured
controls reproduce the reported general thickening of Type 1C output. This
supports testing the actual Windows font/path rasterization and job settings,
without assigning a cause to a Windows component that has not been observed.

The 54x55 QR source contains exactly two RGB colors (black and white), with no
intermediate-gray source pixels. Gray edges in the software renders are created
by rendering/sampling, not by grayscale pixels introduced by the converter.
The low source resolution and noninteger device-pixel mapping are relevant to
sampling; the appropriate Windows sampling behavior is still unmeasured.

The barcode's predictor encoding was also checked. Its Image BitsPerComponent is
1, while its PNG predictor uses the default 8-bit components and Columns=208.
Those predictor rows are byte-stream groups, not necessarily image scanlines:
118 full 208-byte groups plus a final 26-byte group reconstruct exactly 24,570
bytes (26 bytes x 945 image rows). Independent manual Sub-filter decoding matches
Poppler's raw PBM pixels exactly after accounting for PBM's inverse bit convention.
Forcing predictor BitsPerComponent=1 without reencoding fails with invalid PNG
filter bytes. No such metadata rewrite was made. A generic decoder's warning/padding
of the final partial group is not evidence of the company's printer failure.

## Geometry, font and sampling audit

* Independent FontTools parsing compared 52 visible glyphs, 5,106 normalized
  path/control points. Maximum difference is 0.00000732422 in 1000-unit font space:
  about **0.0000000513 pt at 7pt size**. Advances agree after normalization.
* Type 3 FontMatrix `[1 0 0 1 0 0]` and glyph `0.01` transforms are composed into
  the outlines; Type 1C uses conventional 1000-unit coordinates and CFF's default
  0.001 font matrix. This is a unit conversion, not a geometric emboldening.
* CFF paths have no stem-hint/mask operators. Private DICT contains only
  defaultWidthX=0 and nominalWidthX=0; no supplied stem/blue-zone hints or ForceBold.
  The descriptor has no ForceBold flag or synthetic-bold operation. StemV=0 is
  metadata, not a path stroke. **Format conversion still changes the renderer's
  font path:** CFF can receive engine-specific autohinting/antialiasing/stem
  darkening, unlike Type 3 paths. Missing hints do not prove Windows disables these.
* Page content and source Form content remain byte-identical. Both original image
  streams remain **compressed-byte identical**, with dimensions, bit depth,
  colorspace, placement and interpolation unchanged. The 1D image is 208x945,
  1-bit gray at about 300ppi; QR is 54x55 RGB at about 97ppi. Interpolate is false
  or defaults false. The converter does not resample them or invent missing detail.
* Both imposed PDF boxes are 595.28x841.89 pt. At Actual, placement is
  `[1 0 0 1 0.14 -0.055]`: 100% scale with centering for the rounded source A4
  box 595x842 pt. At Fit, scale is 0.9399663866. Fit is preserved deliberately.
  Actual agreement with Acrobat's 100% is established in PDF coordinates only;
  neither the photo nor this host establishes measured printer calibration.

## What can and cannot be localized

The PDF converter preserves geometry and code data. Changing font format causes
small-text rasterization differences even in one renderer, but the measured
changes do **not** reproduce the photographed large increase in weight. The
unchanged heading/code regions also do not reproduce their photographed degradation.
It would be incorrect to declare either "conversion is irrelevant" or "XPS is
confirmed faulty": the Windows renderer may react differently to the CFF format,
and the actual Windows job's route/settings have not been supplied.

The existing path remains: imposed PDF -> isolated compatibility preparation ->
Windows.Data.Pdf -> Direct2D command list -> XPS/validated PrintTicket -> driver.
Default PDF destination dimensions and 96/in DIPs are logical geometry, not proof
of a 96dpi page raster. Direct2D's raster fallback DPI comes from DEVMODE, or 300
when neither axis is reported; it does not prove every object is rasterized there.
This Linux investigation cannot observe Windows intermediate XPS resources,
actual validated job quality/resolution, font substitution/darkening, driver
halftoning, or paper output. Same printer and paper do not establish identical
per-job settings. No native-stage cause is confirmed here.

Accordingly **no runtime printing, font-hint, DPI, scaling, sharpening or image
conversion setting was changed**. The successful `a06fcfcc` behavior is preserved.
Changing those now would be an unverified experiment, not an evidence-based fix.

## Changes and repeatable checks

Added a test-only local oracle in `print_safety/quality_tests.rs`. It exports
paired Actual/Fit PDFs, checks page/Form/image bytes and placement, renders each
pair with pdftoppm at 300 and 600dpi under four explicitly recorded font/vector AA
profiles (FreeType=yes, thinlinemode=none), and records ROI coverage, gray edges, black
run means and exact scanline boundaries. Selected ROIs can demand exact pixels.
Two synthetic unit tests prove that thickened strokes, closed barcode gaps and
gray-edge changes are detected. The module is excluded from product builds.

To reproduce with a private one-page document, create a local ROI JSON file:

```json
[
  {"name":"small_text","rect":[133,283,270,302]},
  {"name":"heading","rect":[115,87,235,101],"exact":true},
  {"name":"barcode","rect":[35,94,78,318],"exact":true},
  {"name":"qr","rect":[240,201.5,280,242.4],"exact":true}
]
```

Set `PDFCRAFT_PRIVATE_PRINT_PDF`, `PDFCRAFT_PRINT_QUALITY_REGIONS` (JSON file), and
optionally `PDFCRAFT_PRINT_QUALITY_REPORT` (local output JSON). Install pdftoppm.
Run `cargo test --profile low-disk -p pdfcraft-engine --lib private_print_quality_pairs -- --ignored`.
No document or derived raster is uploaded by the test. Missing tools/input fail
explicitly; ordinary CI declares the private test ignored rather than silently passing.
The private test now records 64 ROI rows for this four-region fixture; 48 heading/
code rows require exact equality. The independent MuPDF/FontTools/Ghostscript
measurements above are additional local oracle work,
not part of this automated Poppler test.

Validation: scoped engine tests passed **259**, with 6 declared ignored tests;
the 3 private print tests (including this paired oracle) passed separately.
Scoped Clippy with `-D warnings`, formatting, layers, asset and parity checks
passed. Engine WASM compilation passed with 4 pre-existing renderer warnings
in this feature configuration; the desktop-required Korean flag was unset for
the web check. Existing parity coverage warnings remain. Native Windows execution
was not performed.

## Smallest useful Windows follow-up, using the existing successful build

1. Retain `print.prepare` and `print.submit.end` logs. Confirm Actual, A4,
   force_image=false and the Type 1C route; record printer, driver, resolution,
   quality/toner mode and Acrobat scaling. Queue defaults are not validated-job evidence.
2. Using the existing CLI, export the same prepared CFF PDF with a script:

   ```json
   [
     {"tool":"doc_open","args":{"path":"input.pdf"}},
     {"tool":"doc_print","args":{"doc":1,"path":"safe-a4-actual.pdf","paper":"A4","layout":"actual","print_compatibility":true}}
   ]
   ```

   `pdfcraft-cli.exe run --script quality.json --root .`
3. Print both original and exported safe PDF in Acrobat at Actual/A4 with identical
   driver settings. Compare to PdfCraft printing the original at the same settings
   via the Type 1C route. Reopening the safe PDF in PdfCraft adds a further imposition
   wrapper, so do not describe that as byte-identical native input.
4. If Acrobat-original vs Acrobat-safe differs, investigate representation-dependent
   font rendering in that same application. If Acrobat-safe stays clear but PdfCraft
   is heavy, prioritize the Windows rendering/PrintTicket/driver boundary, then inspect
   a synthetic XPS/virtual-printer output before changing native rendering parameters.
   Neither result alone assigns blame to a specific XPS component. Use a ruler/scan
   for physical sizes, keeping Fit as a separate explicit control.

History: 2026-10-29 — controlled v2 audit; no product fix without a confirmed cause.

Follow-up 2026-10-29: recorded tentative 100% recollection and missing Windows
evidence; added explicit AA controls, third-renderer and predictor audits. Runtime
printing, image data, scaling, crash containment and all existing features unchanged.
