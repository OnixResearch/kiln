## Why

The review merge queue (`kiln.review-queue.v1`) is FIFO per target and advances only the head entry of each target per tick. Queue latency is therefore the sum of every candidate's CI time. Agent-authored changes (#3) raise review volume, so serial integration becomes the bottleneck. `kiln-core` `gating.rs` already defines `GateMode::{Serial, Speculative}`, `WindowPolicy`, `admitted_candidate_indices` and `update_window`, but nothing in the review queue uses them (GitHub issue #11).

## What Changes

- **Profile:** `kiln.review-queue.v1` gains an optional `mode` (`serial` or `speculative`) and an optional `window` (`floor`, `ceiling`, `increase`, `decrease`). An absent `mode` is `serial`. `speculative` requires `window`, and `serial` rejects it. Both fields are validated by the Nickel contract in `config/web-code-review.ncl` and by Rust admission.
- **Provider port:** a new versioned `ReviewOperation::BuildSpeculativeIntegration { commit, expected_target, predecessor, strategy }` behind a new `speculative_integration` capability. It builds the exact candidate on top of an assumed predecessor candidate that must descend from the exact current target, and it checks the target with exact compare-and-swap. The local Git adapter implements it. Speculative mode requires the capability, so a provider without it fails at admission, before any external effect.
- **Speculative queue:** for each target, the admitted window (`admitted_candidate_indices`) stacks candidate k on candidates 0..k-1. Canonical CI starts for every exact candidate commit in the window. Submission stays strictly in queue order through the existing PrepareSubmit and Submit commands with exact target CAS.
- **Failure:** a failure is conclusive only at the head. The failing entry is ejected, the later candidates' queued CI runs are cancelled, and results from runs that already started are ignored. The remainder is rebuilt on the current target with fresh CI, and the window shrinks through `update_window`. Each successful submission grows the window.
- **Durable chain:** each speculative entry durably records its assumed predecessor chain: the root target and each predecessor's queue entry, generation and candidate commit, plus a BLAKE3 chain identity. Late integration and CI results for superseded chains are rejected, and the superseded chain identities are retained, within a bound, on the entry.
- **Restart:** restart in the middle of a window reconciles stored candidates, runs and pending provider operations without dispatching them again. CI admission request identities are deterministic per entry, generation, chain and project.
- **Serial unchanged:** serial mode keeps today's code path, identities and serialized bytes. Every new field is serde-default and omitted when it holds its default.
- **Docs:** the queue section of `docs/operator.md` and the profile reference.

## Impact

- **Consumers:** operators running the review merge queue, and authors and reviewers whose changes wait in it.
- **Owners:** Kiln web-service maintainers own the queue shell. Kiln provider maintainers own the review port and the local Git adapter.
- **Affected areas:**
  - `crates/kiln-provider-api/src/change_review.rs`;
  - `crates/kiln-adapter-git/src/change_review.rs` and its conformance tests;
  - `crates/kiln-web-service/src/code_review/{queue.rs,speculation.rs,commands.rs}`;
  - one `State` field in `code_review.rs`;
  - one engine helper in `engine/change_review.rs`;
  - `config/web-code-review.ncl` and its fixtures;
  - `docs/operator.md`.
- **Testing:**
  - unit tests for the pure planner;
  - queue integration tests for each acceptance criterion;
  - adapter conformance tests;
  - Nickel positive and negative fixtures;
  - replay tests that serial profiles and records stay byte-identical.

## Out of Scope

- Cross-target batching, and unchecked batch merges. Every candidate is tested and submitted on its own.
- Provider-side cancellation of CI runs that have already been dispatched. Their results are ignored, and the existing per-project execution capacity bounds them.
- Changing the existing per-project dispatch serialization in the engine. A speculative window admits candidate runs together, and execution parallelism stays bounded by the engine's existing capacity limits.
- Console visualisation of the predecessor chain beyond the existing queue identity facts.
