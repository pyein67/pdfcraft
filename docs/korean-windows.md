# Korean text in PdfCraft 0.8.0 / Windows x64

This change extends the existing PdfCraft app, its Text box Post action and Add Text
editor. English menus, existing PDF editing tools and the optional MCP behaviour stay
as before. Korean uses Nanum Gothic Regular, not a Windows system font and not
Source Han/Noto CJK. The font is OFL-1.1, attributed in ATTRIBUTION.toml; no font binary
is committed to this repository.

## Build on Windows 10 x64

Use Windows 10 x64 (22H2 recommended), Visual Studio 2022 Build Tools with **Desktop
development with C++**, an x64 MSVC toolchain and Windows 10/11 SDK, Rustup, Git and
curl.exe. Open **Developer PowerShell for VS 2022**. GitHub Actions can instead run
`.github/workflows/windows-korean.yml` on a Windows x64 runner; that is a build/test
recipe, not evidence of a Windows 10 runtime test.

From this branch's repository root:

```powershell
./packaging/windows/build-korean.ps1
```

The script installs Rust 1.95.0, selects `x86_64-pc-windows-msvc`, checks out the
existing release's pinned craft-fonts input (`3d9ab3a132ed0eff457e424226a435a901da30d3`)
so the other supported scripts remain available, then downloads the pinned
Nanum Gothic font and its OFL licence into `../craft-fonts-korean`, verifies both
SHA-256 hashes, runs the affected library, automation and UI tests, then builds the
existing `pdfcraft.exe` and `pdfcraft-cli.exe`. Every native command's exit code is
checked. It leaves the current English UI catalogue unchanged.

Outputs (or the equivalent under an existing `CARGO_TARGET_DIR`):

```text
target/x86_64-pc-windows-msvc/release/pdfcraft.exe
target/x86_64-pc-windows-msvc/release/pdfcraft-cli.exe
target/x86_64-pc-windows-msvc/release/NanumGothic-OFL.txt
target/x86_64-pc-windows-msvc/release/KOREAN-ATTRIBUTION.txt
target/x86_64-pc-windows-msvc/release/NOTICE
target/x86_64-pc-windows-msvc/release/craft-fonts-licences/
```

Keep the font licences, program LICENSE-MIT / LICENSE-APACHE and notices with redistributed binaries. These development builds
are unsigned. Font data is embedded inside the EXE and PDF; users do not need to
install Nanum Gothic. `-FontDir <external-directory>` selects an existing craft-fonts
input and preserves its other fonts. `-SkipTests` is available for a rebuild after
validation; do not treat that as a tested build. The release profile can take a while
on the first build. Two compiler jobs limit resource use.

The equivalent manual build sequence is:

```powershell
git clone --no-checkout https://github.com/storytold/craft-fonts ../craft-fonts-korean
git -C ../craft-fonts-korean checkout --detach 3d9ab3a132ed0eff457e424226a435a901da30d3
cargo +1.95.0 run --locked --profile low-disk -p xtask -- korean-fonts ../craft-fonts-korean
$env:CRAFT_FONTS_DIR = (Resolve-Path ../craft-fonts-korean).Path
$env:CRAFT_FONTS_REQUIRED = '1'
$env:PDFCRAFT_KOREAN_REQUIRED = '1'
$env:PDFCRAFT_REQUIRE_WINRES = '1'
$env:CARGO_BUILD_JOBS = '2'
cargo +1.95.0 build --locked --release --target x86_64-pc-windows-msvc -p pdfcraft -p pdfcraft-cli
```

`PDFCRAFT_KOREAN_REQUIRED=1` deliberately fails a build without the Hangul font input,
so a successful generic build without it is not a Korean-enabled deliverable.

## What changes in PDF output

Text box/Callout/Typewriter and Add Text use the existing Unicode font embedding
writer: a Type 0 composite font, CIDFontType2, full TrueType program (`FontFile2`),
CIDToGIDMap, exact glyph advances and ToUnicode. The font program is deduplicated
within a document. `/Contents` keeps typed Unicode; modern decomposed Jamo are
composed for drawing. Search treats composed/decomposed Hangul alike and keeps
correct glyph ranges for highlights. Existing text replacement can select the
Korean face through the existing Type 3 outline fallback with ToUnicode.

The Korean fallback is currently Regular; it does not synthesize bold/italic or
claim complete historical Hangul support. Characters the face lacks produce an
explicit error instead of committing question marks. This does not repair a PDF
that already lost its original Korean characters by saving literal `???`.

## Validation

Automated checks include `korean_text_box_post_save_reopen_search_and_render`,
`korean_added_text_embeds_font_and_keeps_other_items_after_edit`,
`korean_glyphs_exist_in_every_input_font_family`, and
`korean_typing_and_paste_post_save_reopen_and_ctrl_f`. The UI test covers ordinary
text events, clipboard paste, simulated IME preedit/commit, the actual Post button,
PDF serialization/reopening and Ctrl+F. It is an egui harness, not a native Windows
IME, GPU or desktop session.

Before distributing a Windows build, run this manual check on Windows 10 x64:

1. Open an existing PDF in the existing PdfCraft app, keeping the English menus.
2. In Text box, type `한글 입력 저장 검색 ABC 123` using the Windows Korean IME.
   Confirm preedit/commit text contains no replacement boxes. Repeat by pasting.
3. Click Post; confirm the page has real Hangul glyphs, not question marks.
4. Save to a new PDF, close the app, restart and reopen the saved file.
5. Press Ctrl+F, search `한글`, `저장` and `검색`; verify results and highlight boxes.
6. Repeat with Add Text. Edit/move the text and save again; check existing English
   text, page rotation, undo/redo and annotations still work.
7. Open the saved PDF on another machine without Nanum Gothic installed.

Windows 10 native IME and executable runtime validation require a Windows machine;
a Linux test run or cross-target type check does not establish those outcomes.

The current machine’s exact test results and remaining checks are recorded in
[the validation report](korean-validation.md).
