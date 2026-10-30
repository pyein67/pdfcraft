# v0.15.0 Windows print compatibility and initial paper

Historical b51 implementation note; advisory query recovery is described in [the follow-up](v015-paper-query-recovery.md).

Status at b51: local implementation; Windows crash isolation, native media discovery and physical output require Windows validation. No release build, push or PR.

Last reviewed / updated: 2026-10-30.

## Baseline and observed boundary

This branch starts at official `storytold/pdfcraft` tag `v0.15.0`, commit
`75360d1d7db06fed974f48c914d4c04132757187`. Its shared spooler is craft-libs
`e11f9d9aebea5f6b8e74643c1ea1ed172ba5f545`; its Windows PDF path still uses
winprint `5f80b9baf6b58937c401656634e42d33f00ddf49`, Windows.Data.Pdf,
Direct2D command lists and XPS. Although craft-print also depends on a GDI wrapper,
this PDF submission path does not use its pixel transport. The native call stack
of the reported termination has not been captured. Type 3 is a compatibility
routing heuristic, supported by earlier successful physical tests, not a proven
root-cause diagnosis for every native crash.

The attached private one-page label has 595 x 842 pt MediaBox/CropBox, one Type 3
font and two source images (208 x 945, 1-bit grayscale; 54 x 55, RGB). The private
input and all derived PDFs/images stay outside git and published artifacts.

## Minimal compatibility adapter

Preflight is bounded to
256 MiB and 100,000 resource visits / depth 128; inputs beyond these limits are refused,
including a document whose compatibility class cannot safely be determined.
Only Windows submission inspects final imposed sheet resources. PDFs without
Type 3 keep the original shared spooler, bytes, progress callback, options and
rendering engine. Save as PDF, browser and CUPS transports are unchanged.

Type 3 jobs run in a private temporary directory through the same executable's
explicit `--pdfcraft-type3-print-worker` entry (desktop and CLI both install it).
The parent keeps the document open and reports a failed/aborted/timed-out worker
as an error. It never automatically retries a job, because some sheets could
already be queued. Native crashes cannot be caught by Rust catch_unwind; the
process boundary is the protection. A Rust panic inside preparation is also caught.
The child’s temporary-directory environment points inside the parent-owned private folder
so backend job files can also be cleaned after a native abort. No global environment is changed.
A bounded ticket preserves printer, copies, collation, duplex, scaling, colour,
title and driver job options. Native printing still uses the official spooler.
The worker does not forward sheet progress; the existing progress card stays
indeterminate until completion for compatibility jobs only.

The restricted Type 3 -> embedded Type 1C converter and worker helpers are reused
from the project’s `a06fcfcc` implementation after review against the v0.15.0 API.
No old branch is merged. This does not import Korean input/UI/icon changes,
600dpi image fallback, diagnostics libraries, Samsung A/B controls, or vendored
craft-print/winprint patches. No DPI, text-darkening, PrintTicket or DEVMODE override
is added. Ordinary native submissions are unchanged.

Only supported uncoloured path-fill glyphs are accepted. FontMatrix, advances,
encoding, page geometry, text operators, non-font streams and embedded images are
preserved within the converter's explicit bounds (Type 2 operands use 16.16).
Coloured glyphs, unsupported graphics state/paint/text modes and excessive inputs
are refused before native submission. There is no automatic whole-page image
fallback; the user's existing explicit Print as image option is unchanged.
CFF vs Type 3 antialiasing/font handling can differ; preservation is not a claim
that every rendered pixel or a driver's font treatment is identical.

## Paper priority and consistency

The upstream GUI starts with `PrintDraft.paper = 0` (US Letter); `restore` replaces
it with a recognized saved canonical `print_paper`. This is an application initial
value, not evidence of a driver forcing Letter. Upstream `persist` writes that
value even without an explicit paper change, so a legacy saved Letter cannot be
distinguished from a deliberate Letter choice and is respected.

1. Any valid existing saved paper retains priority, including Letter or label stock.
2. With no recognized saved paper, a Korean Windows system UI locale (`ko`, `ko-KR`,
   or underscore form) starts with A4. Other systems and headless defaults stay as before.
3. On Windows the desktop queries the selected printer's media off the GUI thread,
   through the existing pinned safe winprint wrapper. If the desired paper is not
   advertised, the active preview/job uses the driver's validated empty-ticket
   paper if available, otherwise the first valid advertised paper. Custom driver
   dimensions are supported for that fallback. The desired selection is not
   overwritten or persisted as a fallback; another printer can use it again.
4. Unreadable/empty capabilities stop submission with an error. No unsupported A4
   is forced. In-flight capability results for another printer are ignored.

The displayed paper label, preview layout and generated PDF sheets use the same
`effective_paper()` resolution. Native submission derives its exact PrintTicket
paper from these sheets with the unchanged craft-print driver validation; an
unexpected driver paper/scaling/orientation substitution is refused by that backend.
No queue/default settings are written and no Preferences window is opened by this
read. Existing Fit/Actual choice, margin and scale calculations remain unchanged.
A4 Fit still reserves 18pt margins and can shrink a matching A4 document (~94%);
this change prevents an initial Letter mismatch, not Fit's intentional reduction.

## Reproducible verification

Source tests: `cargo test -p pdfcraft-print --lib`; UI print tests:
`cargo test -p pdfcraft-ui-egui --test it print::`.
The pure locale resolver tests simulate Korean Windows on the host; that is not
a Windows UI execution. The native read-only capability adapter and native worker
printing still require Windows execution. UI tests cover saved choices, supported
A4, label-only fallback, pending/failed reads, preview/prepared sheet/job geometry,
and existing print workflows. Worker tests simulate nonzero exit, SIGSEGV and a
stuck child without contacting a printer. Synthetic tests cover nested fonts,
normal-byte identity, unsupported glyph refusal, crop/rotation/spacing and original
image stream preservation.

The ignored `compatibility::tests::local_private_pdf_vector_preparation` accepts
explicit local input/output environment paths and never prints. It verifies
source boxes and unchanged image streams and produces a new local converted file.
A private MuPDF 1.26.6 600dpi grayscale comparison gave:

| Region | Changed pixels | Maximum gray difference |
| --- | ---: | ---: |
| QR image | 0 | 0 |
| 1D barcode image | 0 | 0 |
| Entire 4959 x 7017 page | 30,578 | 183 |

Whole-page mean absolute error was 0.04735 on a 0–255 gray scale; integrated ink
coverage changed -0.00717%. The nonzero glyph differences remain a limitation.
These metrics compare the original PDF and converted PDF with the same renderer,
not the Windows output engine, a printer or Acrobat. No physical success or
Acrobat-equivalent print quality is claimed by automated checks.

Windows validation: open on a Korean system without a saved paper; check A4 on an
A4-capable queue, saved Letter/label choices and switching to a label-only queue;
confirm the dialog and physical stock/size agree. Print the private label and an
ordinary PDF at the same selected Fit/Actual setting as the comparison application.
Verify the parent stays open, no duplicated/partial jobs after failures, small
text/stem widths, barcode spacing, QR boundaries and actual physical scale. Do not
change driver darkness/DPI while comparing. Approval is required before push,
Windows build or distribution.

## Completed host checks

| Check | Result |
| --- | --- |
| pdfcraft-print library tests | 63 passed; 1 optional private-input test ignored in the ordinary run |
| UI integration print workflows | 29 passed |
| UI internal print tests, including asynchronous printer changes | 9 passed |
| Existing headless printing tests | 3 passed |
| Explicit private-input preparation (raw + imposed Fit/Actual) | 1 passed; no printing |
| Pinned craft-print shared regressions | 60 passed on host |
| Print crate all-target Clippy, warnings denied | passed |
| Formatting, layers, assets, parity | passed (parity retains upstream missing-tool notes) |
| wasm32-unknown-unknown print crate check | passed |

The shared regression run used an unmodified copy of the eight source files from the
actual pinned dependency, with only workspace manifest inheritance made standalone.
No library source, dependency revision, Windows engine or driver setting was changed.
The Type 3 and A4 regression tests were observed failing after temporarily removing
those respective implementations, then passing after restoration.

Strict whole-graph UI Clippy is **not clean in the release baseline**: unchanged
`crates/edit/src/images.rs` and `crates/forms/src/appearance.rs` report two diagnostics.
With `--no-deps`, unchanged `comments_panel.rs`, `app/files.rs`, `combine_ui/actions.rs`
and `compare_ui.rs` report six diagnostics. These files were verified byte-identical
to v0.15.0. The supplemental selected-crate all-target check passed using
`--no-deps -- -D warnings -A clippy::unnecessary_option_map_or_else
-A clippy::collapsible_match -A clippy::nonminimal_bool`.
These three existing lint categories were excluded for that command only; no source
allow attribute, Cargo lint configuration or unrelated implementation was changed.
This result is separate from the strict print-crate check and functional test passes.

## Changed source areas

- `crates/print/src/compatibility.rs`, `compatibility/outlines.rs`,
  `compatibility/worker.rs`, `compatibility/tests.rs`: bounded inspection, vector conversion,
  isolated submission and regression/private-oracle tests.
- `crates/print/src/paper.rs`, `paper/tests.rs`: locale priority and read-only media resolution.
- `crates/print/src/lib.rs`, `spool.rs`, `Cargo.toml`, root `Cargo.lock`: facade connection,
  existing pinned safe wrapper and Windows-only protocol dependencies.
- `crates/print/src/tests.rs`: expose the existing synthetic fixture to sibling tests only.
- `crates/ui-egui/src/workflows/print_ui.rs`, `print_ui/draft.rs`, `print_ui/body.rs`,
  `src/lib.rs`, `src/app.rs`: async media read, effective sheet selection and matching UI label.
- `crates/ui-egui/tests/it/print.rs`: persistence, capability-failure/fallback and sheet consistency;
  existing workflow fixtures explicitly choose Letter so their tests do not depend on OS locale.
- `apps/pdfcraft/src/main.rs`, `apps/pdfcraft-cli/src/main.rs`: early worker opt-in entry;
  Windows desktop connects read-only media discovery.
- Print README, this verification note and the two roadmaps: scope and remaining native validation.

No input, save/search, translation catalog, icon, workflow or packaging source was changed.
