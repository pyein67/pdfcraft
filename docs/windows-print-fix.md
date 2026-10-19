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

The native diagnostic artifact also includes `print-fix-icon-metadata-ab.zip`: a copy of
the reported bad release with **only 14 bytes** changed, the low byte of each of fourteen
resource-group plane words. `metadata-probe.json` records both EXE hashes and file offsets;
its script checks that every other EXE byte is unchanged. This optional A/B probe preserves
the bad release's code, settings, PNG pixels and red icon, isolating the metadata hypothesis
from recompilation and from the separate dialog fix. It is not the rebuilt preview and is
not evidence of successful paper output. Only the user's same-printer A/B result can confirm
or refute that hypothesis. The normal preview ZIP below is rebuilt from the modified source.

## Narrow changes in this preview

* All seven supplied ICO entries have `wPlanes=0`; the previous working lion and document
  icons have `wPlanes=1`. Both versions use PNG frames, so “PNG versus DIB” is NOT the
  observed difference. The supplied PNGs have valid CRCs. Some PNG icon loaders accept zero;
  this is **not evidence that the icon caused an access violation**.
* A separate Windows ICO copy normalizes just these seven plane words to 1. All offsets,
  dimensions and PNG bytes remain identical. The original uploaded ICO and lion icons stay
  in the repository. Window/taskbar PNG pixels, red PDF design, PDF association and resource
  IDs remain unchanged. Unit tests and the native PE check enforce the single-plane metadata
  and exact frame bytes. Win32's icon loader is exercised on the built EXE.
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

The icon metadata policy test fails on the previous red ICO and passes on the normalized
copy. A separate test proves that only plane words changed. UI tests cover delayed discovery,
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
OUTPUT_VERIFIED, FAILED and NOT_RUN (missing CI driver). Real Samsung output must be listed
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
