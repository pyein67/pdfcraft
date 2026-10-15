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
