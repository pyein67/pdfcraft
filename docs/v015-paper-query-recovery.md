# v0.15.0 advisory paper-query recovery

Status: local source/tests only; no push or Windows build. Native failure on the company PC is not yet identified by an API/HRESULT.
Last reviewed / updated: 2026-10-30. Base: `b51d26d95c807287a050f3bc0c88ee65e8971ee7` (official v0.15.0 plus the existing print fix).

## Confirmed regression and evidence limit

Official v0.15.0 does not query supported media before enabling Print. The dialog lays out its selected paper; the existing native backend reads capabilities and validates the real job's paper, orientation and scaling after submission.

The b51 change introduced an asynchronous `paper::capabilities()` read and made it a prerequisite:

1. The new read enumerates devices, fetches/parses PrintCapabilities and probes an empty/default PrintTicket via builder open/merge/build.
2. Every failure, including failure of that optional default-paper probe, returns an error and discards an otherwise valid media list. Outer `to_string()` conversion omits nested HRESULT details.
3. `effective_paper()` maps a pending read or an error to an invalid draft. The preview fails and the Print button is disabled.
4. The paper combo converts that error to `None` and displays `Reading printer paper…` for both pending and failed reads.

The reported preview text `Printer paper could not be read…` is emitted only for an error result (including a disconnected worker), not for the pending state. Thus the reported persistent Reading label is a confirmed display/error-policy regression, not proof that the Windows call is still running.

A4 factory selection itself is a pure locale-to-preset lookup and calls no driver API. Its implementation and saved-paper restore are unchanged here. The added prerequisite query, not that pure lookup, introduced the UI blockage. The generic b51 message cannot identify whether enumeration, provider open, PTGetPrintCapabilities, XML parsing, optional default merge or worker failure actually failed on the company PC. Failure of the empty/default merge is a concrete unnecessary fatal dependency, **not an established diagnosis** of the observed native error. No COM initialization, driver, firmware, DPI or media-support cause is assumed.

## Minimal scoped changes

- Media discovery is advisory. While pending, failed, timed out, disconnected or unavailable, preview/Print use the user's selected paper (or the unchanged Korean-Windows A4 factory preset) with an explicit unverified-paper notice. That is not a claim that the printer supports it.
- Known capabilities still resolve a supported selected size or a valid advertised/default fallback. A temporary fallback never overwrites the remembered paper.
- The existing native `driver_ticket` matching and PrintTicket validation remain the final gate. Unsupported/substituted paper is refused before native page output. If capabilities also fail inside that unchanged backend, the print can still fail; this fix does not claim to repair a native driver/provider failure.
- An optional default-ticket failure is logged without discarding valid advertised media. An unreadable/empty media catalog stays unknown.
- A 10-second UI deadline ends the pending state. A late result cannot replace a timed-out result or satisfy a new request, even after reopening the same printer. No per-frame automatic retries occur.
- A native Windows API call inside a Rust thread cannot be forcibly cancelled safely. Timed-out receivers remain tracked, with at most two outstanding advisory reads per app. Further reads report busy instead of creating unlimited threads; UI preview/Print remain usable. Reopening Print retries when capacity is available.

Type 3/Type 1C conversion, its isolated worker, spool submission, the shared print engine, Fit/Actual, DPI, input/search/save, translations, icons, workflows and packaging are unchanged. No printer/queue defaults are written.

## Read-only diagnostic evidence for the next Windows check

Normal app logging (`warn,pdfcraft*=info`) records `print.paper` events. Portable mode uses `PdfCraftData/logs/pdfcraft.log` beside the executable. A custom RUST_LOG filter can suppress them.

The native adapter records begin/error/completion phases, counts and available HRESULTs:
`devices.enumerate`, `device.select`, `capabilities.open`, `capabilities.stream`, `capabilities.get`, `capabilities.read`, `capabilities.parse`, `media.list`, `defaults.open`, `defaults.merge`, `defaults.stream`, `defaults.read`, `defaults.build`, `defaults.parse`.

UI records carry a request ID, native-thread correlation, elapsed time, result relevance, timeout/worker-stopped/busy status. Optional default-probe errors have `optional=true`. Driver-supplied error strings/BSTRs, raw XML, printer names, paths and PDF data are not recorded by these new events. `windows-core` is the same 0.62.2 error type already used by winprint, added directly only to downcast enumeration error sources for HRESULT logging. No dependency version is upgraded.

For a returned error, its phase and HRESULT identify the failing wrapper/API stage. For a blocked call, the last native begin event plus the UI timeout narrows the outstanding function; a nested blocked call cannot yield an HRESULT until it returns. Tests use synthetic HRESULTs solely to check formatting, not as company-PC observations.

No new Windows binary was built in this task. Therefore actual native failure attribution still requires these diagnostics from a separately approved future Windows test.

## Local verification

- Print library: **66 passed**, one existing opt-in private-input oracle excluded from the normal run.
- UI print internals (including deadlines, stale results, bounded reads and deselect/reselect): **14 passed**.
- UI print integration: **30 passed**, including failed advisory discovery with visible A4, a usable preview and an enabled Print button. No Print click or printer job was performed.
- Negative controls: restoring the old UI gate, making optional-default failure fatal, and removing deadline handling each caused its matching regression assertion to fail. All were restored before final checks.
- Print all-target Clippy with warnings denied, formatting, layers, assets and the wasm print check passed. UI all-target Clippy passed with the same three previously documented baseline lint categories excluded on the command line only; no lint configuration was changed.
- Dependency versions/checksums, the A4 factory function, saved-paper restore, Type 3 conversion/worker, spool facade and engine paths were checked against b51 and preserved. Only existing `log` and `windows-core` dependencies are now directly referenced by the print crate.

These are host-side tests and synthetic read failures. They do not establish which Windows driver/API failed, validate the new Windows-only diagnostic adapter at runtime, or prove physical printing/quality. No GitHub push, Actions execution or Windows build was performed for this follow-up.
