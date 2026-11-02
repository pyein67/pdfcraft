# Optional SumatraPDF printing and Korean Windows A4 defaults

Status: local candidate; Windows build, viewer lifetime and physical printing require separate approval/acceptance.
Reviewed: 2026-11-02. Base: `05bf537aac32c91ab84743e4b960b3ea2985b2d9` (the existing Korean custom build, not official v0.15).

## Preserved source

The branch includes the complete base history: Korean embedding/input/save/search, Korean TSV catalog and UI language preference, existing supplied icon assets and Windows resource/registration code, conditional Type 3 compatibility worker, Type 1C/image fallbacks, diagnostics and opt-in Samsung A/B code. These implementations/assets are not changed. Windows initially selects SumatraPDF and remembers the last chosen method; Fit retains its existing 18 pt margin. No DPI, antialiasing, Samsung driver option or printer default changes.

## External print route

In Print on Windows choose **SumatraPDF (외부)**, enter the full path to an independently installed `SumatraPDF.exe`, and click **SumatraPDF 인쇄 창 열기**. The viewer is not bundled, installed or downloaded by PdfCraft. The executable path and print method are remembered. With no saved method, Windows starts with SumatraPDF; an existing saved Native choice remains Native. Other platforms keep the native factory default. Missing/invalid saved method values use the factory default. An invalid executable produces the existing error and does not switch methods.

This route commits active form typing, checks the existing print permission, then uses `Session::external_print_pdf` / incremental `save_bytes`. An unedited PDF is passed byte-for-byte; unsaved edits are appended without marking the document saved. The original file is not overwritten. No imposition, Type 3 → Type 1C conversion, image conversion, or barcode/QR resampling is performed by this route. Existing font/image/content streams survive incremental serialization. Edits already made to those objects intentionally affect the snapshot. Password-protected snapshots retain encryption: SumatraPDF may ask for the password again; no password is put on the command line. Pending annotation text still needs the normal Post action, while active form typing follows the existing commit path.

A randomly named temporary directory holds the PDF and an isolated SumatraPDF settings profile. That profile disables instance reuse, history/session persistence and update checks. Installed SumatraPDF settings and printer queue defaults are untouched. Arguments are passed separately to `Command`, without a shell. The worker waits for viewer exit; normal success and error clean up the temporary directory. Abrupt termination of PdfCraft/the OS can leave temporary files behind. No PDF content, title, original path or password is logged by the new route. Closing the viewer is not proof of physical printing: cancel and success are not distinguished by a zero exit code; nonzero exit/spawn errors are shown to the user.

**The SumatraPDF dialog owns printer, paper, page range, copies, orientation, colour, duplex and scaling.** The native PdfCraft controls and native preview are hidden on this route, rather than suggesting those settings were applied. Select A4 only if supported and select Actual/100% in the external dialog when making a physical comparison. PdfCraft's saved paper is retained for the native route; it is not sent to the external dialog.

Official public CLI documentation: <https://www.sumatrapdfreader.org/docs/Command-line-arguments>. `-print-dialog` / `-exit-when-done` open and finish the interactive route. `-print-settings` including `paper=A4` is documented with direct-print commands; this candidate deliberately does not use those commands. Profile isolation uses documented `-appdata` and `ReuseInstance=false`: <https://www.sumatrapdfreader.org/docs/Advanced-options-settings>. No SumatraPDF/MuPDF implementation code is copied or linked. Test installed viewer versions (3.2+ for `-new-window`) on Windows before claiming supported process lifetime.

Headless API/tool: `doc_print_external` with `doc` and exactly one of `preview_path` (snapshot only, no printing) or `executable` (Windows interactive viewer). All user paths honour `--root`. The viewer is launched only by an explicit tool call/UI action. No new service/control channel starts.

## A4 priority and asynchronous media discovery

1. An explicit persisted user paper choice, including custom stock, wins over locale. The old base did not persist paper at all; a choice from an already closed base-version session cannot be recovered from nonexistent data.
2. With no saved choice, Korean Windows user locale prefers A4. Other environments retain Letter. Factory defaults are not saved as explicit user choices.
3. For a known printer media catalog, resolve the desired size to a supported size, otherwise the driver's advertised default or first valid advertised paper. A temporary fallback never overwrites the user's saved choice. The menu offers advertised custom stock and disables known unsupported presets.
4. Media discovery is advisory. Pending/failed/timed-out reads keep the selected preview and Print usable, with a visible notice. The existing native backend still validates actual job paper, orientation and no-extra-scaling before native output; unknown media is not treated as supported. If native driver validation also fails, the job can still fail without output.

The native media adapter and bounded query state machine are selectively reused from `8c4388f0`, not the rest of v0.15.0. Default-ticket probe failure does not discard valid advertised media. A 10-second deadline ends the UI pending state. At most two outstanding reads are tracked even after timeout; stale results cannot overwrite a new printer/reopened request. Switching printer refreshes its media and preview. No `Reading printer paper...` gate exists.

Preview and native submission use the same `effective_paper` dimensions. The existing craft-print exact-sheet PrintTicket validation is preserved; driver substitutions are still refused. Actual device acceptance and physical measurements are not established by synthetic tests. `print.paper` stages/HRESULTs go to normal app logs (portable: `PdfCraftData/logs/pdfcraft.log`); no special switch, raw XML, printer names or PDF content in these new events.

## Print-method preference follow-up

Based on `87dc9fa03dbf1e60a37b2644dd2ad9b511e8a8ad`, isolated on `fix/remember-print-method`. The only product changes are the Windows factory method and the `print_method` settings field (`native`/`sumatra`). Existing `sumatra_exe` persistence, route implementations, error handling, paper policy, rendering and assets are unchanged. Selection survives cancelling/reopening Print and restarting through the existing app settings persistence; there is no automatic native fallback. No push or Windows build for this follow-up.

Local preference verification: 383 UI unit tests passed (one existing raster-measurement probe ignored), plus 17 Print integration tests passed. The original sandbox blocked three existing loopback transport tests; rerunning with local networking allowed passed the full suite. The new tests cover both saved methods, missing/invalid settings, Korean executable paths, real Ctrl+P after reopen/restart, and empty/missing executables without method changes or printing. A negative control that removed method restoration failed the preference regression; the implementation was restored before final checks.

## Original implementation verification

Host checks (Rust 1.95, locked dependencies, required existing Korean font):

| Check | Result |
| --- | --- |
| Print library, including media policy and mock external process | 36 passed |
| Engine library, including edited snapshot and denied-print permission | 254 passed, 4 existing opt-in/private/timing tests ignored |
| Automation integration, including existing Korean save/search and snapshot/root confinement | 160 passed |
| Print UI unit tests, including advisory failure, deadlines, stale results, persistence | 13 passed |
| Real shell Print integration | 15 passed |
| Existing Korean UI typing/Post/save/reopen/Ctrl+F and menu/control regressions | 3 passed |
| Clippy, touched crates/all targets, warnings denied | Passed |
| Formatting, layers, assets, parity validation | Passed (parity retains existing tool-coverage advisory) |
| WebAssembly UI check | Passed (3 existing native-helper dead-code warnings) |

Two negative controls were actually executed: returning stale saved bytes made the edited-snapshot regression fail; removing Korean-Windows A4 initialization made the locale regression fail. Both changes were restored and the positive tests rerun. Native/external dialog screenshots were rendered and inspected locally. The supplied problem PDF was exported via the new CLI snapshot tool without printing: all 154,771 bytes matched its input exactly. Package versions and checksums in Cargo.lock are unchanged; only dependency edges/direct references changed.

 Linux synthetic tests do not prove Windows IME, Windows print/provider behavior, SumatraPDF cancellation/lifetime or hardware quality. No GitHub push, PR or Windows build is performed in this task.
