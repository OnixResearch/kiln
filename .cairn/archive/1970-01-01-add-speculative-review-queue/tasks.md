## Phase 1: Port and profile

- [x] [serial] Add `BuildSpeculativeIntegration` and the `speculative_integration` capability to the review port and implement them in the local Git adapter with conformance tests. r[kiln.review.queue.speculative.provider] r[kiln.review.queue.speculative.provider.scenario.unrelated]
- [x] [serial] Add `mode` and `window` to the queue profile with Rust admission, the Nickel contract and positive and negative fixtures. r[kiln.review.queue.speculative.profile] r[kiln.review.queue.speculative.profile.scenario.invalid]

## Phase 2: Speculative queue

- [x] [serial] Add the pure planner in `code_review/speculation.rs`: chain identity, chain validity, CI verdict, window admission and update. r[kiln.review.queue.speculative.chain] r[kiln.review.queue.speculative.window]
- [x] [serial] Add the speculative shell: stacked builds, concurrent CI admission, in-order submission, ejection, supersession with queued-run cancellation, late-result rejection and durable windows. r[kiln.review.queue.speculative.window] r[kiln.review.queue.speculative.failure] r[kiln.review.queue.speculative.chain]

## Phase 3: Verification and docs

- [x] [serial] Add queue integration tests for the window of three, the failing middle entry, the restart, the late result, serial byte identity and advisory approvals. r[kiln.review.queue.speculative.window.scenario.three] r[kiln.review.queue.speculative.failure.scenario.middle] r[kiln.review.queue.speculative.chain.scenario.restart] r[kiln.review.queue.speculative.chain.scenario.late] r[kiln.review.queue.speculative.serial] r[kiln.review.queue.speculative.serial.scenario.replay]
- [x] [serial] Update the queue section of `docs/operator.md` and record the verification evidence. r[kiln.review.queue.speculative.profile] r[kiln.review.queue.speculative.serial]
