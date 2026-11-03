# Windows print regression — diagnostic preview

Updated 2026-10-19. **Actual Samsung paper output is unverified. The crash's faulting
module/exception has NOT been established. This preview is not a claim of a complete fix.**

## Confirmed history and source comparison

| Role | Source commit | Windows build |
|---|---|---|
| Last working lion-icon Korean UI, according to the user's hardware observation | `76dd98efc4687a49cee5399c3539a82e58b09be6` | Actions `38830255662`, artifact `12019784784` |
| First icon change / regression candidate | `47b99dfe23ed116b7010dc314dd88ce40025ce62` | First resource change |
| Released red-icon version, reported to crash | `a49b1991c86fc57e750f31e68f5b2099c6bbb2ca` | Actions `38858842640`, artifact `12031331235` |

The user's description identifies these release versions; the exact source SHA on their
computer should be confirmed with the log/build information if another binary was used.
`git bisect start --no-checkout a49b1991 76dd98ef` selects `47b99dfe` as the next test.
No hardware bisect verdict was fabricated: Samsung printing is unavailable here.
`a49b1991` changes only the icon test, workflow concurrency and documentation after `47b99dfe`;
it changes no production application source. The persistent-download maintenance commit
`af362582` adds only a release-upload workflow and does not change the executables.

`git diff 76dd98ef a49b1991 -- crates/print crates/engine crates/fonts crates/annot
crates/edit crates/render crates/ui-egui Cargo.toml Cargo.lock` is empty. The only app runtime
source difference is the Windows window-icon byte constant. The EXE's two icon resources,
installer shortcut icon, capabilities icon registration, tests, notices and packaging changed.
The PDF open command and ProgID resource ID `-2` were retained. Both Windows build scripts
use Rust 1.95, MSVC x64 and the same font input. No CRT/static-linking, manifest, renderer,
print engine, PrintTicket, DPI, document settings or dependency change appears in that diff.
The original build script did not automatically execute the optional registration script.

The diagnostic Windows job downloads the two original ZIPs using pinned source identities
and their original SHA-256 values. It records actual EXE hashes, imports, subsystem/security
flags, manifest hash, version information, icon directories and Win32 icon-loading results.
Matching source build settings alone are not substituted for this binary inspection.

Independent inspection of the original released ZIP (SHA-256
`efe76c7f25c65fb23b5f1f876f09fd0e5b6677d42e310ba4ae4008bd789c5ce0`)
confirms EXE SHA-256 `81d86f4a135edc4878050b7922f05f8e9636ea8798d76e5493ba2ecafce576a0`
and that **all fourteen compiled RT_GROUP_ICON plane words are already 1**.
The uploaded ICO has seven zero plane words, but the resource compiler normalizes them.
Both old and new icons use PNG frames and the supplied PNG CRCs are valid. A zero-plane
source-header hypothesis was investigated and **ruled out for this actual released EXE**.
An experimental metadata-only probe refused this real input as designed. The speculative
normalized ICO, its tests and its probe script were removed; no icon or build-resource
change is included in the final source. The original red icon, pixels, resource IDs,
window/taskbar PNG, PDF association and backups are preserved exactly.

## Narrow changes in this preview

* Printer discovery was already asynchronous. The first unknown selection (`None`) was
  rendered as “Save as PDF”, then replaced by the discovered default. Clicking that already
  selected `None` value did not mark an explicit user choice; reopening also reset the choice.
  The dialog now shows its existing localized “Looking for printers…” text, blocks submission
  until discovery or an explicit choice, and preserves explicit PDF/printer selections.
* Logs record `print.prepare`, `print.submit.begin`, `print.submit.end` and
  `print.submit.error` with settings/byte counts and returned error text. A begin without an
  end narrows the native boundary but is NOT a crash stack. Document content is not logged.
* The existing native print backend is retained. No replacement PDF editor, helper print
  program, raster-print fallback, printer-model workaround, dependency update, native code
  rewrite or process-isolation redesign is included.

## Existing Windows print path

PdfCraft imposes pages into a print-ready PDF. Save as PDF stops after writing that PDF.
A named queue instead calls the pinned `craft-print` backend and `winprint` wrappers:
`EnumPrinters`/default selection → PrintCapabilities and PrintTicket merge/validation →
DEVMODE conversion via the driver → Windows.Data.Pdf + Direct3D11/Direct2D → XPS print
package target → terminal completion/cancellation status. Provider calls, rendering,
callbacks and native object destruction happen inside the application process; a native
access violation cannot be caught by ordinary Rust Result or catch_unwind.

This selected backend does not use PdfCraft's page renderer to allocate full-page printer
bitmaps or issue a GDI `CreateDC`/`StartDoc`/`StretchDIBits` page loop. Direct2D's raster
fallback DPI comes from the driver DEVMODE. Existing geometry, paper/orientation checks,
colour/duplex/copy settings, scoped submission thread and temporary PDF lifetime are all
unchanged from the working release. These facts do not exonerate a driver or prove a fault
in it; an event/dump is needed to distinguish application, Windows rendering and driver code.

## Tests and limits

UI tests cover delayed discovery,
default selection, explicitly choosing PDF while pending, reopening, cancellation and PDF
save failures. A Korean automation test creates and imposes three pages, reopens the result
and checks search, embedded TrueType and ToUnicode. Existing Korean input/Post/save/reopen/
Ctrl+F and full CI suites remain enabled. A passing print-ready PDF test is not a spool test.

The dedicated Windows job executes the **unchanged pinned dependency's** native API tests,
including mandatory jobs to its own paused Generic/Text queue. This exercises the Windows
submission path without a Samsung device. Its logs are kept as `print-fix-native-diagnostics`.
If a CI printer/GPU setup prevents submission, that failure must be reported, not called
successful paper output. A separate native Microsoft Print to PDF probe attempts English, Korean and three-page
output to temporary file ports and reopens the PDFs. Its JSON report distinguishes
OUTPUT_REOPENED, FAILED and NOT_RUN (missing CI driver). Native driver PDFs are not published without a font provenance audit; only JSON/text
reports leave the runner. Reopened page count does not verify printed glyph appearance.
Real Samsung output must be listed
separately from both virtual PDF output and API/paused-queue tests. Windows Server 2022 CI is not Windows 10 hardware.

## Windows 10 acceptance and crash evidence

1. Keep the working lion build in a separate folder. Do not overwrite it. Extract this
   `print-fix` ZIP to another folder and open the same PDF from within the program first,
   avoiding file-association changes during the comparison.
2. Test the same English PDF, Korean PDF and three-page PDF on each Samsung queue using the
   same paper/orientation/copies/colour settings. Record time, selected queue and driver
   version. Confirm actual sheets, page count, Hangul and margins; a submitted job is not proof.
3. Test Microsoft Print to PDF separately, save its output, reopen it and check all pages.
   Test opening Print, changing selection, explicitly selecting Save as PDF, reopening and
   cancelling. Confirm Korean input/paste/Post/save/reopen/search and the red icons remain.
4. If printing still closes the app, run `Collect-PrintDiagnostics.ps1` in PowerShell. It reads
   Application Error (1000), Windows Error Reporting (1001), hang (1002) and available
   PrintService logs plus OS/printer/driver information. It changes no queue or default.
   Keep PdfCraft logs from its settings folder and the exact failure time. Reports can contain
   printer/host/document names: review/redact before sharing. Do not put private PDFs in Git.
5. Event Viewer → Windows Logs → Application: preserve the faulting application/module,
   exception code and fault offset, plus WER Report ID. “Stopped working” alone is insufficient.
6. If the event is insufficient, enable WER LocalDumps **only for pdfcraft.exe**, in an elevated
   PowerShell. Back up any existing per-app WER settings first and restore them afterwards:

```powershell
$folder = Join-Path $env:LOCALAPPDATA 'PdfCraft\CrashDumps'
New-Item -ItemType Directory -Force $folder
$key = 'HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\pdfcraft.exe'
reg add $key /v DumpFolder /t REG_EXPAND_SZ /d $folder /f
reg add $key /v DumpType /t REG_DWORD /d 2 /f
reg add $key /v DumpCount /t REG_DWORD /d 2 /f
```

Reproduce once and keep the dump private: full dumps can contain PDF contents and account
information. In WinDbg, `!analyze -v`, `.ecxr`, `k` and `lm` identify the exception thread,
module and stack. Share the redacted stack/module/code first. Do not upload full dumps to a
public issue. Remove only the settings you added, or restore the original exported settings.

## Separate baseline CI advisory failure

The unchanged Cargo.lock and advisory policy reproduce three maintenance notices under
`cargo deny --all-features check`: RUSTSEC-2026-0370 (`zvariant`),
RUSTSEC-2026-0371 (`zbus_names`) and RUSTSEC-2026-0372 (`zvariant_derive`).
These are unmaintained-crate advisories in the existing Linux accessibility dependency
chain, not a new printing or icon dependency. The audit reports no safe upgrade;
licenses, bans and sources pass. The notices are not suppressed and the pinned native
printing dependencies are not upgraded during this regression investigation.

## Verified native virtual-output baseline (2026-10-19)

The original lion and red-icon ZIPs were downloaded again, their pinned hashes and source
identities verified, and their existing CLI binaries each submitted English, Korean and
three-page documents to the inbox Microsoft Print to PDF driver. All **six outputs reopened
with the expected page counts**, in Actions run `39193660660`, job `117871648905`.
This establishes virtual output on that CI host, not Samsung paper output or glyph appearance.
The original native backend also passed the mandatory paused-queue API tests in run
`39185795024`, job `117848039121`.

Run `39185795024` successfully completed the Korean core/font, automation, UI/language-switch,
Post/save/reopen/search tests, release compilation, and native icon/registration/GUI-launch
checks. Its subsequent initial virtual-output probe tried to reopen a file as soon as it
existed and reported “no readable page tree or cross-reference data”. The probe now waits up
to 30 seconds for a nonempty file that actually reopens, preserving the page-count assertion
and recording stdout, stderr, attempts and failure metadata. This is a test-harness correction,
not a speculative rewrite of printing. Native driver PDFs remain private to the runner.

A packaging/probe-only follow-up may reuse the successful Korean source-test step from
`39e85680` only after Git proves that **every other tracked file is unchanged** and the GitHub
API verifies the exact tested source and successful step. Any program/test/font/lockfile/build
input change forces the full Korean tests. Full workspace CI, new-EXE icon/registration/GUI
checks and real native virtual-output probes still run. The preview ZIP includes the actual
`NATIVE-PRINT-PROBE.json`; failed probes keep a failing workflow outcome even when a compiled
diagnostic preview is retained for investigation.

PdfCraft logs are normally `%APPDATA%\PdfCraft\data\logs\pdfcraft.log`; a portable copy uses
`PdfCraftData\logs\` beside its EXE. Preserve the current and rotated logs after a crash.
