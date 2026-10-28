# PR #1 CI follow-up

The Korean patch commit is `2b4469568730084f7af0eef4dd3733f150af164c`;
its base is `ac89a113915ea890999cf1a141cdfcda1ad06d98`.

## Initial failures (2026-10-15)

The public GitHub job pages show that both CI test jobs stopped at **Clippy**,
before their test steps. The dependency job stopped in cargo-deny.
Raw step logs are unavailable in this cloud environment: `api.github.com`
is denied by the network proxy and anonymous web log requests return 404.
The diagnostics below were reproduced locally, rather than read from those
remote step logs.

These are baseline problems, separately fixed from the Korean feature:

- `forms/src/af.rs`: `clippy::collapsible_match` in literal date parsing.
  A match guard preserves the existing optional-literal behavior.
- `oracle/src/pixels.rs`: Rust 1.99 stable reports
  `clippy::chunks_exact_to_as_chunks`. Fixed-size RGBA array chunks preserve
  the decoder's checked bounds and pixel calculation.
- The unchanged original lockfile selected rustls 0.23.45, affected by
  RUSTSEC-2026-0346, -0347, -0348 and -0349. Cargo's precise update to
  0.23.46 fixes all four without suppressing advisories. Cargo also resolves
  six Windows dependency edges to the already locked windows-sys 0.60.2.

Both lint sites exist in the base commit. The Korean feature commit has no
changes to Cargo.toml or Cargo.lock. No lint, license or advisory policy is
relaxed.

## Local checks

- Rust 1.99.0: workspace/all-targets Clippy with `-D warnings` passes.
- `cargo deny --all-features check` (cargo-deny 0.20.2): advisories, bans,
  licenses and sources pass after the update.
- Existing `af::tests::dates_times_and_masks`: passes (1 test).
- Formatting, dependency layers (37 crates), assets pass.

Full workspace tests, wasm checks and the GitHub rerun are recorded separately
when complete. A successful cross-build does not establish interactive Windows
10 IME behavior; native acceptance remains necessary as described in
[korean-windows.md](korean-windows.md).

## First rerun

Commit `3d080d7e` passed remote Clippy on both platforms, wasm, cargo-deny and
packaging lint. Ubuntu then failed at the workspace test step (exit 101).
Locally, the full workspace suite with Korean fonts and required qpdf/Poppler
completed: 3,096 passed, zero failed, 14 ignored. The local duplicate wasm
command timed out including its build-lock wait; the remote wasm job passed.

To diagnose the remote-only test failure, CI now preserves the real Cargo exit
code while adding the last 200 output lines to the job summary on failure and
uploading the complete test log. The Korean Windows build does the same for
its PowerShell exception and build output. Neither wrapper turns a failure
into success or changes which tests run.

The failed Linux test log was uploaded in run `38686392322` as `test-log-Linux`
(artifact `11950643633`), but anonymous access also hides step summaries and
artifact downloads here. The final 60 lines are additionally emitted as an
escaped GitHub error annotation, which the public job page does expose.

## Korean-patch regression identified from actual remote output

Run `38688008163`, Ubuntu job `116267281403`, exposes the failure in its
annotation: `creating_from_text_refuses_what_the_font_cannot_draw` failed at
`automation.rs:2717`, reporting unsupported `A` (`U+0041`) for
`ABC 제안서 시험`; the existing test correctly expects `제` (`U+C81C`).
The automation suite completed with 156 passing and one failing test.

This **is a Korean-patch regression**, distinct from the baseline lint and
security issues above. Without a bundled Korean face, `first_undrawable`
tested every character against that absent face, incorrectly rejecting even
ASCII. It now uses the Korean face only when one is present; otherwise the
existing standard-font validation reports the first genuinely unsupported
character. The existing regression test and its expected Unicode value are
unchanged. Korean-enabled builds still use the embedded Unicode font.

An additional local Rust 1.99 check using the unmodified CI font input passed
all 598 core tests before this correction; it did not include the automation
test that exposed the problem. The earlier full 3,096-test run used Korean
fonts, which explains why it did not expose the missing-font path.

After the correction:

- Rust 1.99 with the unmodified CI font input: all 157 automation tests pass,
  including the unchanged failing test; all 132 edit library tests pass.
- Rust 1.95 with Korean fonts required: all three Korean automation regressions
  pass (Text box Post/save/reopen/search/render, Add Text, existing text edits).
- Scoped Rust 1.99 edit/automation all-targets Clippy with `-D warnings`,
  formatting, edit wasm32 check, layers and assets pass.
