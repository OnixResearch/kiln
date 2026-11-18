## Phase 1: Contract

- [x] [serial] Specify the fixture, its outcomes and series boundaries in the proposal, design and delta spec. r[kiln.admission.queue_wait]

## Phase 2: Load and core

- [x] [serial] Add the Nickel load contract, the frozen positive and a negative fixture, the committed export and its freshness check. r[kiln.admission.queue_wait.scenario.load]
- [x] [serial] Add the pure load admission, classifier, summary and receipt core with read-back validation and positive and negative unit tests. r[kiln.admission.queue_wait.scenario.series] r[kiln.admission.queue_wait.scenario.load]

## Phase 3: Harness

- [x] [serial] Add the shared harness, the off-by-default example and the integration test over the real review queue in serial and speculative modes, with the frozen expectation. r[kiln.admission.queue_wait.scenario.outcomes]

## Phase 4: Documentation and evidence

- [x] [serial] Document the fixture and its boundaries, update the changelog, run the focused checks and retain one receipt as evidence. r[kiln.admission.queue_wait]

Verification is recorded in `evidence/verification.md`, and the retained receipt
is `evidence/receipt.json`. No admission, queue or budget default changed, and no
production or external merge-queue measurement is part of this change.
