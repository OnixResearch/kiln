## ADDED Requirements

### Requirement: Queue mode and window profile

r[kiln.review.queue.speculative.profile]
The `kiln.review-queue.v1` profile MUST accept an optional `mode` of `serial` or `speculative` and an optional `window` with `floor`, `ceiling`, `increase` and `decrease`. An absent `mode` MUST mean `serial`. `speculative` MUST require a bounded window, with `1 <= floor <= ceiling <= 16` and `ceiling <= max_entries`, and the `speculative_integration` provider capability. `serial` MUST reject a window. The Nickel contract and Rust admission MUST enforce the same rules, and existing inputs MUST export unchanged.

#### Scenario: Invalid speculative profile

r[kiln.review.queue.speculative.profile.scenario.invalid]
- GIVEN a speculative queue profile without a window, with `floor > ceiling`, or without the `speculative_integration` capability
- WHEN the profile is admitted
- THEN admission MUST fail before any provider effect or CI dispatch

### Requirement: Speculative provider integration

r[kiln.review.queue.speculative.provider]
The review port MUST offer `BuildSpeculativeIntegration` behind the `speculative_integration` capability. It MUST build the exact candidate on an assumed predecessor that strictly descends from the exact expected target, and it MUST fail when the target moved. Its request identity MUST include the predecessor, and existing operation identities MUST NOT change.

#### Scenario: Predecessor not on target

r[kiln.review.queue.speculative.provider.scenario.unrelated]
- GIVEN a predecessor commit that does not descend from the expected target
- WHEN a speculative integration is requested
- THEN the provider MUST reject it without writing an operation journal ref

### Requirement: Speculative window

r[kiln.review.queue.speculative.window]
In speculative mode, for each target, the queue MUST admit positions through `admitted_candidate_indices`. It MUST build candidate k on candidates 0..k-1, start canonical CI for each exact candidate commit without waiting for earlier submissions, and submit strictly in queue order through fresh PrepareSubmit and Submit with exact target CAS.

#### Scenario: Three passing entries

r[kiln.review.queue.speculative.window.scenario.three]
- GIVEN window 3 and three approved entries on one target
- WHEN the queue advances
- THEN three candidate CI runs MUST be admitted before the first submission, and the target MUST advance through the candidates in queue order

### Requirement: Failure ejection and window adaptation

r[kiln.review.queue.speculative.failure]
A CI or admission failure MUST eject only the failing entry once it is head. Later candidates built on it MUST be superseded: their queued CI runs cancelled, the results of already-started runs ignored, and each rebuilt on the current target with fresh CI. The window MUST shrink on failure and grow on success through `update_window`.

#### Scenario: Failing middle entry

r[kiln.review.queue.speculative.failure.scenario.middle]
- GIVEN three speculative entries where the middle candidate fails CI
- WHEN the queue advances
- THEN only the middle entry MUST be ejected, and the last entry MUST be rebuilt, pass fresh CI and land after the first

### Requirement: Durable predecessor chain

r[kiln.review.queue.speculative.chain]
Each speculative entry MUST durably record its assumed predecessor chain, with a BLAKE3 identity, in the same state commit as the queue. Integration or CI results for a superseded chain MUST be rejected. Restarting mid-window MUST reconcile the stored candidates, runs and pending operations without dispatching CI or provider operations again.

#### Scenario: Restart mid-window

r[kiln.review.queue.speculative.chain.scenario.restart]
- GIVEN a window with three candidates and admitted CI runs
- WHEN the service restarts and the queue advances
- THEN the same candidates and runs MUST be kept, no CI MUST be dispatched again, and submissions MUST complete in order

#### Scenario: Late result for superseded chain

r[kiln.review.queue.speculative.chain.scenario.late]
- GIVEN an entry whose chain was superseded after its CI run started
- WHEN that run later succeeds
- THEN the result MUST NOT qualify or submit the rebuilt entry

### Requirement: Serial compatibility and authority

r[kiln.review.queue.speculative.serial]
Serial mode MUST behave byte-identically to the previous queue: the same identities, the same statuses, and serialized profiles, queue entries and journals without new keys. In every mode, a submit MUST require current approvals, owners, policy, dependencies and full/clean CI for the exact candidate. Source approvals MUST NOT transfer, and advisory approvals MUST NOT count.

#### Scenario: Serial replay

r[kiln.review.queue.speculative.serial.scenario.replay]
- GIVEN a profile, queue entry and pending journal serialized before this change
- WHEN they are loaded and serialized again, and a serial queue runs to submission
- THEN the bytes MUST be identical and the stored records MUST contain no speculative keys
