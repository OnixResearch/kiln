## Why

Issue [#87](https://github.com/OnixResearch/kiln/issues/87): a selected campaign stops whenever its bounded driver runs out of admissible work. `drive-implementation-campaign` drives once for an exact expected store version and exits on an uncertain effect, a no-effect wait, a replan, a block, completion or its operation limit. Nothing resumes the campaign when a review is answered, verification finishes, the source moves or capacity returns. An operator must notice and start the driver again with the current version. A canonical admission refusal (another work unit wins, the repository writer is busy, a budget is exhausted) also ends the driver with an error, so capacity waits look like failures.

## What Changes

- Add a pure continuation core in `kiln-core`:
  - **Wake hints** (`review`, `verification`, `source`, `capacity`, `reconciliation`, `integration`, `operator`) coalesce into per-campaign demand with a monotonic sequence. They never grant effects, resources or completion.
  - **Decision:** a continuation step either drives, waits until a hint, a version change, a backoff deadline or a bounded rescan, or stops at completion, cancellation or an exhausted failure budget.
  - **Classification and backoff:** each driver batch is classified, and failures back off exponentially under a bounded budget.
- Persist one continuation record per campaign in the existing campaign database, merged in immediate transactions. A hint recorded while a batch runs stays pending. The record holds no work queue or progress; selection, claims, fences, effect intents and reconciliation stay with their owners.
- Add `kiln_implementer::campaign::continue_bounded`, which runs at most a configured number of driver batches over the existing `drive_bounded` loop. The drive loop now reports a canonical capacity refusal as a typed capacity wait instead of an error; exact admission reports a busy repository writer as typed `writer_busy` instead of an error.
- Add `kiln-web-service continue-implementation-campaign --profile FILE --campaign-id ID`, a one-shot step for a supervisor to invoke, and `kiln-web-service wake-implementation-campaign --database DB --campaign-id ID --kind KIND`. Resolving a decision or continuing admission through the campaign operator records a wake hint in the same transaction.
- Add an optional `continuation` policy to the drive profile; existing profiles keep named defaults.

## Impact

- **Consumer:** operators and the onix-core supervision of selected campaigns (#49 owns timers and units).
- **Immediate outcome:** answered reviews, owner observations and recovered capacity resume eligible campaign work without a manual restart; waiting campaigns do not busy-loop.
- **Durable capability:** one tested continuation core and loop shared by the native controller and scripted owners.
- **Owner:** Kiln implementation and web-service maintainers.
- **Files:** `crates/kiln-core/src/implementation/{continuation.rs,mod.rs}`, `crates/kiln-implementer/src/{admission.rs,campaign.rs,campaign_consumer.rs,campaign/continuation_tests.rs,campaign/drive_tests.rs}`, `crates/kiln-implementer/tests/admission.rs`, `crates/kiln-web-service/src/{implementation_campaign.rs,implementation_campaign/continuation.rs,main.rs}`, `config/implementation-campaign-drive.ncl`, `docs/implementation-runbook.md`, `CHANGELOG.md`, `cairn-policy/development-coverage.ncl` and this package.
- **Verification:** core and implementer unit tests with scripted owners over the real SQLite store; web-service tests; Nickel gate; workspace and Nix gates; Cairn gates, validation and traceability.
- **Authority:** working-copy implementation and verification only. No real worker, model, provider or remote effect, and no supervisor deployment.

## Out of Scope

onix-core timers, path units and deployment (#49). Authenticated web commands and a decision inbox (#86). Typed Cairn snapshot observations (#83). Authority binding (#69/#70). Owner-originated notification transports: owners and supervisors call the wake command or rely on the bounded rescan.
