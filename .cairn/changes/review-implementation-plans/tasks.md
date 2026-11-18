## Phase 1: Contract

- [x] [serial] Specify the opt-in policy, pinned plans, annotations, gate, feedback, comparison and frozen receipt in the proposal, design and delta spec. r[kiln.implementation.plan_review] r[kiln.implementation.plan_review.comparison]

## Phase 2: Core

- [x] [serial] Add the pure plan artifact, region, annotation, resolution, decision, gate, feedback and comparison core with positive and negative unit tests. r[kiln.implementation.plan_review.scenario.revision] r[kiln.implementation.plan_review.scenario.refused]
- [x] [serial] Bind the opt-in policy and review threads to work-ledger eligibility without changing the default ledger or its stored form. r[kiln.implementation.plan_review.scenario.default] r[kiln.implementation.plan_review.scenario.blocked]

## Phase 3: Store, CLI and frozen comparison

- [x] [serial] Add the transactional store command and the `plan-review` and `plan-feedback` operator commands, with durable claim and CLI tests. r[kiln.implementation.plan_review.scenario.blocked]
- [x] [serial] Add the Nickel case contract, positive and negative fixtures, the checked export, the replayed receipt and its committed expectation. r[kiln.implementation.plan_review.comparison.scenario.frozen]

## Phase 4: Documentation and evidence

- [x] [serial] Document plan review and its boundaries, update the index and changelog, run the focused checks and record evidence. r[kiln.implementation.plan_review] r[kiln.implementation.plan_review.comparison]

Verification is recorded in `evidence/verification.md`. The committed receipt
expectation is `fixtures/plan-review/expected.json`. The default admission path
is unchanged, and no captured agent plan or measured quality gain is claimed.
