## What this changes

Adds 한국어 to the existing interface-language registry and embeds a clean-room Korean catalog for menus, tools, dialogs and notices. Auto resolves ko-KR through the existing Windows display-language detection; an explicitly saved language continues to take priority. Missing translations continue to display English.

Based on `fix/korean-input-save-search`, including its existing Korean PDF font embedding and search support. Neither that branch nor main is modified. The existing Korean Windows build installs the pinned Nanum Gothic font and now runs localization and language-switch regressions too.

## How you verified it

See [Korean UI validation](https://github.com/pyein67/pdfcraft/blob/feat/korean-ui-localization/docs/korean-ui-validation.md) for exact local results and limitations. CI runs the full workspace on Ubuntu and Windows. The dedicated Windows x64 Korean build tests with the Korean font required and uploads an unsigned EXE ZIP with font licences.

- Automated input, paste and simulated IME → Post → save → reopen → Ctrl+F regression runs in both English and Korean.
- Catalog validation checks placeholders, contexts, plurals, command labels, source literals and dynamic catalog keys. Glyph coverage checks every translated character in every UI font family.
- Settings tests drive the real interface through the opt-in control channel, switch 한국어 / English and search translated commands while preserving document state and command IDs.
- Actual Windows 10 GUI/physical IME acceptance requires a Windows 10 machine; a Windows Server 2022 CI build is not that acceptance test.

## Provenance and safety

Uses the existing localization and font paths; no new runtime network access, unsafe code, proprietary string tables or font binaries. The catalog is contributor-original under MIT OR Apache-2.0 and attributed in ATTRIBUTION.toml. PDF editing and saving implementation is unchanged.
