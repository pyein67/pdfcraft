# Korean UI localization validation

Based on Korean PDF branch commit `21ba9975f48b98556203f9d3e09ff2246158d3fa` (PdfCraft 0.8.0). Work is isolated on `feat/korean-ui-localization`.

The existing TSV catalog, runtime language preference, Windows display-language detection and English fallback are reused. Choose Menu > Edit > Preferences… > Interface language > 한국어 or English. In Korean: 메뉴 > 편집 > 환경 설정… > 인터페이스 언어. Changes apply immediately and persist. Auto follows supported system locales, including Korean Windows display languages; an explicitly saved language wins. `ui.set` accepts `ko` and `en` without changing document text or command IDs.

The Korean catalog has 2,592 entries. OS errors remain unchanged inside translated messages. Product names, keyboard key caps and technical identifiers remain in their original form. Untranslated future strings fall back to English. No external translation service is used at runtime.

Use the existing `packaging/windows/build-korean.ps1` from a Visual Studio 2022 Developer PowerShell with C++ desktop tools and Windows SDK. It installs Rust 1.95, verifies the pinned external Nanum Gothic font and OFL, runs core/automation/UI regressions and builds x64 desktop and CLI EXEs. Font bytes are embedded; no installed Korean font is needed for this package. Keep all packaged licences and NOTICE beside redistributed EXEs. The GitHub Actions artifact is `pdfcraft-0.8.0-windows-x64-korean-ui-unsigned` and downloads as ZIP.

## Results

Local Linux validation (Rust 1.95, pinned external fonts with Korean required):

| Check | Result |
|---|---|
| UI library | 375 passed; 1 pre-existing raster measurement probe ignored |
| UI comments | 35 passed; Korean typing/paste/simulated IME → Post → save → reopen → Ctrl+F in both `en` and `ko` |
| UI control | 58 passed; settings switch in both directions, menu labels, command search and document-state preservation |
| UI fonts | 11 passed; embedded Korean glyph checks executed |
| Engine library | 234 passed; 1 pre-existing ignored test; Korean calendar and numeric dates included |
| Korean automation regressions | 3 passed; existing text replacement, Text box disk save/reopen/search/render, Add Text |
| Clippy | touched crates, all targets, `-D warnings`: Rust 1.95 and Rust 1.99 passed |
| WebAssembly UI compatibility | passed (`wasm32-unknown-unknown`, Korean-required flag unset); dead-code warnings in unchanged native-only helpers remain |
| Formatting / assets / layers / parity | passed; 271 repository assets and 37 crates |

Total of these suites: 716 passed, 0 failed, 2 existing ignored tests. The full workspace is delegated to the normal Ubuntu/Windows CI workflow.

The catalog checks commands, tool groups, direct/literal UI lookups and all source/context keys in the bundled catalogs. Its placeholders, ellipses and plural forms are validated. The glyph regression actually failed on `▸`; changing that separator to `>` made the complete catalog render without missing glyphs in all four UI font families. Two ellipsis mismatches were also caught and corrected before the final passing run.

The real egui control harness rendered menus and Preferences at logical widths 1024/1280/1440 and scales 100/125/150%, and inspected control bounds. Local screenshots (system fonts disabled) were visually inspected, including document properties and password protection. No missing Korean glyphs or clipped menu labels were found in those samples. This is sampled visual evidence, not a claim to have inspected every UI state.

No Windows 10 physical IME, live Korean Windows language auto-detection or interactive clipping acceptance has been performed. Native CI uses Windows Server 2022. Headless UI tests use egui events, including simulated IME events, rather than a Windows input method. There has been no independent linguistic review.

Commands used (with the existing environment's Rust/tool paths initialized):

```sh
export CRAFT_FONTS_DIR=/workspace/craft-fonts CRAFT_FONTS_REQUIRED=1 PDFCRAFT_KOREAN_REQUIRED=1
cargo test --locked --profile low-disk -p pdfcraft-ui-egui --lib --test fonts --test comments --test control --features eframe/x11
cargo test --locked --profile low-disk -p pdfcraft-engine --lib
cargo test --locked --profile low-disk -p pdfcraft-automation --test automation korean_
cargo clippy --locked --profile low-disk -p pdfcraft-ui-egui -p pdfcraft-engine --all-targets --features eframe/x11 -- -D warnings
cargo fmt --all --check
cargo xtask assets
cargo xtask layers
cargo xtask parity --partial
```

Rust 1.99 Clippy was separately run with the original non-Korean CI font input and the Korean-required flag unset. Screenshot probes set `PDFCRAFT_SYSTEM_FONTS=0` only for their individual processes; setting it for all library tests disables an installed-font test's prerequisite. WebAssembly excludes the Korean font as before; its compatibility check must likewise unset the Windows-specific `PDFCRAFT_KOREAN_REQUIRED` flag. This task does not add Korean glyph delivery to the web build.
