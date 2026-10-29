# Windows compatibility print quality candidate

Based on `39a764704d58b03d8987e03ca95f2b9a4c77a075`, on
`fix/pdf-print-quality`. Physical Windows acceptance is pending. The user reports
that 39a76470 prints the formerly crashing document, but its fine text and codes
look heavier/less clear and its label appears smaller than another reader's output.
The photo is not a calibrated size or resolution measurement.

## Geometry and sampling audit

| Stage | Geometry and sampling |
| --- | --- |
| Source | Private repro: one unrotated page; MediaBox and effective CropBox `[0 0 595 842]` pt (209.903 × 297.039 mm). |
| Imposition | Existing `pdfcraft-print::layout`: Actual = 1.0; Fit/Shrink reserve 18 pt (6.35 mm) on **each** edge. The source CropBox, rotation and placement are composed into a Form, not a second page-size guess. |
| A4 output | Existing paper preset is 595.28 × 841.89 pt (210.002 × 297.000 mm). Both the old image PDF and its print-ready vector input have this box. This <0.1 mm box difference cannot explain a substantially smaller label. |
| Fit | With the repro and A4, scale = min(559.28/595, 805.89/842) = **93.9966%**. With Letter (the existing initial preset), Fit = min(576/595, 756/842) = **89.7862%**. The photographed job's settings are unknown. |
| Old compatibility image | Final sheets rendered at 600/72 pixels/pt in 2048 px tiles: A4 raster 4961 × 7016. Lossless 8-bit RGB/Flate, no JPEG, `/Interpolate false`. Tiles share a pixel grid and collectively cover the original sheet dimensions exactly. Pixel-ceiling rounding changes sampling by <1 pixel, not page dimensions. |
| Windows PDF stage | Pinned craft-print f03ec27 / winprint 5f80b9b: Windows.Data.Pdf -> Direct2D command list -> XPS. The renderer uses default destination dimensions. `PdfPage.Size` is supplied to AddPage in Windows DIPs (96/in), distinct from PDF points (72/in). A validated PrintTicket requests matching physical paper, orientation and no driver scaling. PdfCraft's composed jobs use Exact. |
| XPS/driver | Direct2D print control takes the driver's DEVMODE resolution (max of reported axes), or 300 dpi when neither axis is reported. This is a raster-fallback setting, **not evidence that every image is resampled to 300 dpi**. Windows/driver interpolation, fallback use and the actual imageable area cannot be measured on this Linux host. Hardware margins clip exact jobs; native ShrinkToFit would additionally fit to the driver's imageable area, but the desktop uses Exact. |

At 600 dpi, the independent reference render's dark-ink bounds span 2120 × 2083
pixels, about 89.75 × 88.18 mm at Actual size. The outline result differs by one
edge pixel. These are raster-measured content bounds, **not a measurement of the
company printer's actual imageable area or physical output**.

For a same-size comparison, select A4 and **Actual size / Match page at 100%**
in PdfCraft and the other reader. Fit remains a deliberate existing choice;
this patch does not silently undo it or change normal jobs. Measure the paper
and a known printed distance with a ruler, not the photo's perspective.

## Change

Automatic Windows Type 3 jobs still run in the same isolated executable worker.
Before resorting to a full-sheet image, it attempts a restricted outline conversion:

* Uncoloured (`d1`) glyphs containing only initial affine transforms and one
  nonzero fill of move/line/cubic/close paths become embedded Type 1C (CFF/Type 2).
  The repro satisfies these conditions: 53 glyph procedures, 52 fills, no glyph images.
  Its Type 3 FontBBox is `[0 0 1 -1]`, with reversed vertical limits. The generated
  CFF uses an outline-derived valid box. This is a compatibility anomaly, not a
  confirmed explanation of the original native crash (no fault stack is available).
* Glyph coordinates are transformed into conventional 1000-unit font space.
  Fractional 16.16 charstring operands preserve outlines; advances, original code
  encoding and any ToUnicode mapping are retained. No replacement font, artificial
  bold, thresholding, sharpening or higher DPI is introduced.
* Page/content/form/image streams and placement matrices stay unchanged. Other
  text, lines, barcodes, QR artwork and images remain their original PDF constructs.
  This avoids forcing these elements through the 600 dpi full-page image stage.
* The entire attempted conversion is discarded for unsupported constructs,
  non-fill text rendering modes, alias encodings, remaining Type 3 resources,
  unreadable content or safety limits. The existing 600 dpi lossless image path
  is then used. Explicit **Print as image (600 dpi)** still always forces that path.
* Input/output remain capped at 256 MiB. Outline inspection is capped at 100,000
  objects, 32 fonts, 16 MiB total decoded content/procedures and bounded coordinates.
  Native failures/timeouts still cannot terminate the editor; no automatic retry.

Normal Windows jobs keep the old submission path. Korean input/save/search,
locales, icons, imposition defaults, native transport and dependencies are unchanged.
The only UI change is extra layout/force-image diagnostic information in existing logs.
Worker results record the selected outline/image route and fallback reason.

Headless inspection uses the existing `doc_print` tool:

```json
{"tool":"doc_print","args":{"doc":1,"path":"safe.pdf","paper":"A4","layout":"actual","print_compatibility":true}}
```

Default saved PDFs are unchanged. `print_as_image: true` overrides this optional
export setting. Existing permissions, atomic writes and `--root` still apply.

## Verification and remaining acceptance

Synthetic tests cover curved contours/holes, nested forms, advance normalization,
text spacing/TJ, font matrices, CropBox/rotation, byte-identical non-font content,
ordinary-byte pass-through, explicit image printing and conservative fallbacks.
Existing worker crash/timeout tests remain. The private file and derived outputs
are local only; they are never committed or sent to CI.

Independent FontTools decoding accepts all 53 CFF glyphs. Poppler at 600 dpi
produces the same 4959 × 7017 page raster before/after; mean absolute grayscale
error is about 0.085/255, with 0.083% of pixels differing by more than 20 levels.
Ink counts differ by about 0.12%; these metrics establish close reference fidelity,
not a physical print-quality score. Both original image XObjects retain their
decoded bytes, placement, bit depths and effective resolutions (1D image about
300 ppi/1 bit; QR image about 97 ppi/8-bit RGB). No missing source detail is invented.

The transformed synthetic fixture exposed an existing Hayro Type 3 placement
difference. Its new CFF result agrees with independent MuPDF ink bounds; automated
checks for that fixture use exact content/box preservation and normalized advances
instead of treating the incorrect Type 3 render as a visual oracle. The renderer
itself is unchanged.

Final host results: **606** core/print/font/automation/CLI tests passed (5 ignored),
**492** selected UI tests passed (1 existing ignored), and both private-file tests
passed separately. The vector-retention regression was observed failing when the
automatic path was temporarily restored to image-only behavior, then passed with
the fix restored. Scoped Clippy with `-D warnings`, formatting, dependency layering,
asset attribution and parity checks passed. WASM compilation passed with 3 existing
UI warnings, using the unchanged web-font policy (the Windows-only required-Korean
build flag was unset for that check). Existing parity coverage warnings remain. Windows runtime,
physical dimensions, barcode/QR scanning, paper/driver settings and comparison to
other reader output still require Windows hardware testing. An embedded CFF avoids
Type 3 at the native boundary but does not establish the original crash's cause or
guarantee identical driver rendering. Explicit image printing remains the escape hatch.

History: 2026-10-29 — restricted vector compatibility candidate; no GitHub push or
Windows executable build authorized by this task.

## Second quality audit

The user reports successful Type 1C output with remaining heaviness/blur. See
[the controlled v2 comparison](pdf-print-quality-v2.md): geometry and code data
are preserved; reference-renderer measurements do not reproduce the photographed
weight increase. The v2 branch adds tests and analysis without a runtime print change.
