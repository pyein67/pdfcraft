# Korean support validation — 2026-10-15

Baseline: PdfCraft 0.8.0, commit `ac89a113915ea890999cf1a141cdfcda1ad06d98`.
Current validation ran on Linux x86_64 with Rust 1.95.0, two Cargo jobs and the
`low-disk` profile. `CRAFT_FONTS_DIR` used the release's pinned craft-fonts revision
`3d9ab3a132ed0eff457e424226a435a901da30d3`, plus the verified Nanum Gothic input.
`PDFCRAFT_KOREAN_REQUIRED=1` was set; absence of the Korean face fails the build/tests.

## Completed checks

| Check | Result |
|---|---|
| `cargo test --locked --profile low-disk -p pdfcraft-annot -p pdfcraft-edit -p pdfcraft-fonts -p pdfcraft-render --lib` | 41 + 132 + 67 + 272 = **512 passed**, 0 failed, 0 ignored |
| `cargo test --locked --profile low-disk -p pdfcraft-automation --test automation` | **157 passed**, 0 failed, 0 ignored |
| `cargo test --locked --profile low-disk -p pdfcraft-ui-egui --features eframe/x11 --test comments --test fonts` | 35 + 11 = **46 passed**, 0 failed, 0 ignored |
| `cargo test --locked --profile low-disk -p xtask --bin xtask` | **48 passed**, 0 failed, 0 ignored |
| Changed-crate Clippy, all targets, `--no-deps`, `-D warnings` | Passed for fonts, annot, edit, render, automation, ui-egui and xtask |
| CLI build, `cargo build --locked --profile low-disk -p pdfcraft-cli` | Passed |
| `cargo fmt --all -- --check`, `git diff --check` | Passed |
| `xtask assets --write`, `xtask assets` | Passed: 270 repository assets, 6 bundled entries, 23 fetched entries |
| `xtask layers` | Passed: 37 crates, no layering violations |
| `xtask parity` | Passed; existing automation-coverage notes remain |
| `xtask korean-fonts` repeated on the same input | Passed; manifest SHA-256 unchanged, no duplicate font entry |
| Windows MSVC x64 `cargo check` for fonts, annot, edit and render | Passed (type/build check only; no executable or runtime test) |
| Required Korean font omitted | Correctly failed with the expected missing-font build error |
| Windows workflow YAML | Parsed and script path checked; workflow not executed |

Total Rust runner result: **763 passed, 0 failed, 0 ignored** across the selected
targets. Existing tests for other optional script faces can return early when that
face is absent (for example Hebrew); this is not a claim that every optional font
workflow was exercised. The Korean regression paths ran with the required face.
A full workspace sweep was not run; repository guidance scopes local checks.

## Functional evidence

- Before the PDF fix, the new Korean Text box test failed: a Korean search returned
  0 matches. After the fix it passed with the saved TrueType program and ToUnicode.
- Temporarily removing just the Korean UI font registration made the input glyph
  test fail; the registration was restored and the complete UI targets passed.
- The actual egui Text box composer was driven with text events, clipboard paste
  and simulated IME preedit/commit. The test clicks Post, serializes the PDF, reopens
  it, presses Ctrl+F, types into the focused find widget and verifies matches for
  `한글`, `입력`, `저장` and `검색`.
- Add Text was saved/reopened after changing one of two Korean items, with undo/redo;
  both items and their search matches survived. Decomposed Jamo input is covered.
- Existing text replacement preserved Korean through save/reopen, including search
  across the normal line wrap. All 11,172 modern Hangul syllables have font coverage.
- Saved test PDFs were rendered and visually inspected with PdfCraft and independent
  Poppler. Both show real Hangul, not replacement boxes or question marks. Poppler's
  `pdffonts` reports NanumGothic as CID TrueType / Identity-H, embedded **yes**,
  Unicode **yes**. Poppler also extracts the Add Text Korean correctly.
- Local evidence PDFs and PNGs are under `dist/korean-validation/` (generated,
  ignored, not committed); they contain only synthetic test text.

## Limitations and remaining Windows validation

The full Windows application cross-check stopped in the existing `ring 0.17.14`
native build because this Linux machine has no MSVC **lib.exe**:

```text
error occurred in cc-rs: failed to find tool "lib.exe": No such file or directory
```

No Windows EXE was produced, and neither the PowerShell script nor its GitHub
Actions workflow has been run on Windows. Windows 10 native IME, clipboard, GPU,
executable startup and actual desktop behaviour remain unverified. Follow
[the Windows build procedure and manual acceptance test](korean-windows.md).
The egui IME events above do not establish native Windows IME behaviour.

Strict Clippy including unselected workspace dependencies stopped on an existing,
unchanged `clippy::collapsible_match` warning in `crates/forms/src/af.rs:628`.
Clippy with `--no-deps` checked every changed crate and all its targets with
`-D warnings` and passed. No unrelated forms code was changed or lint disabled.

The web/WASM and other operating-system builds were not validated in this change. The Korean face is
Regular; no synthetic bold/italic or complete historical Hangul support is claimed.
Already-corrupted literal `???` in an old PDF cannot be recovered by this change.
