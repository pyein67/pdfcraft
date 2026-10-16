# Windows Korean UI PDF icon build

Updated: 2026-10-16. Based on Korean UI commit `76dd98efc4687a49cee5399c3539a82e58b09be6`.

The contributor supplied `PdfCraft_Korean_PDF_Icon.ico` and confirmed that GPT generated it.
The supplied ICO is stored unchanged as `assets/app-icon/korean/pdfcraft-pdf-icon.ico`.
Its original 256px PNG frame is extracted byte-for-byte for the Windows window/taskbar.
Both EXE icon resource IDs (1: application, 2: PDF document) use the same ICO.
The MSI's shortcuts also use this ICO; the existing PDF ProgID refers to resource 2.
Original lion icons and document icons remain unchanged under `assets/app-icon/`.
The original source is retained on `feat/korean-ui-localization` at the commit above.
Linux/macOS icons, Korean text/PDF handling and translation catalogs are unchanged.

## ZIP use and PDF association

Extract the ZIP to a permanent folder, then run `pdfcraft.exe`.
For optional current-user Open with / Default apps registration, run PowerShell in that folder:

```powershell
powershell -NoProfile -ExecutionPolicy RemoteSigned -File .\Register-PdfCraft.ps1
```

The script exports existing registration keys to a unique `registration-backup` folder
before writing. Preserve that folder. An export failure stops before registration writes.
Backups contain shared registration keys: review them before restoring with `reg import`,
especially if other applications have changed their registration since the backup.
Do not move the EXE after registration; rerun the script if its location changes.
No administrator rights are needed for this ZIP registration.
Choose **PdfCraft** for `.pdf` in **Settings > Apps > Default apps** to use this design
for PDF files. Existing defaults and Windows' protected `UserChoice` are preserved.
Windows caches icons; an Explorer restart/sign-out may be needed to refresh them.
The ZIP and EXEs are unsigned.

## Verification

`apps/pdfcraft/tests/windows_icon.rs` checks all seven sizes and exact PNG frame identity.
`packaging/windows/test-korean-icons.ps1` checks the actual x64 PE's icon resource groups
against every original ICO frame, exercises registration twice in an isolated HKCU subtree,
checks Unicode/spaced open commands and existing default preservation, and launches the
actual GUI to open a synthetic PDF at a Unicode/spaced path through the opt-in control channel.
The existing Korean font, PDF editing, automation and UI regression suites remain enabled
in `build-korean.ps1`. The build fails if any required check fails.

Local validation before push: the new ICO/frame test passed; the existing Korean UI
library (375 passed, 1 existing ignored), comments (35), control (58) and font (11)
tests passed with the Korean font input. The comments suite includes English/Korean
input/paste/Post/save/reopen/Ctrl+F. App all-target Clippy with `-D warnings`, formatting,
asset hashes, dependency layers and wasm UI check passed (wasm has 3 existing warnings).
The PowerShell resource checker passed a synthetic PE and rejected its corrupted icon.
All Windows packaging scripts parsed using PowerShell 7.4.6 on Linux.
These local checks do not replace the native Windows checks described above.

Native Actions runs on Windows Server 2022, targeting `x86_64-pc-windows-msvc`.
A physical Windows 10 Explorer/taskbar and default-app selection are separate manual
acceptance checks; CI does not establish that those have been performed.

Build locally using Visual Studio 2022 Developer PowerShell with Desktop development
with C++, the Windows SDK and Git installed:

```powershell
.\packaging\windows\build-korean.ps1
```

History: 2026-10-16 — Windows-only supplied icon integration and native packaging probes.
