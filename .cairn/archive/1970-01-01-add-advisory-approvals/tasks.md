## Phase 1: Domain

- [x] [serial] Add `AdvisoryApprove`/`AdvisoryWithdraw` commands, the separate `Change.advisory` collection, admission, historical semantics and the advisory search filter in `kiln_core::change_review`. r[kiln.review.advisory] r[kiln.review.advisory.scenario.lifecycle] r[kiln.review.advisory.scenario.forbidden] r[kiln.review.advisory.authority]

## Phase 2: Shell, console and observation

- [x] [serial] Admit advisory actions for read principals only, expose advisory lists and the search filter, and render the labelled non-binding list in the console. r[kiln.review.advisory.view] r[kiln.review.advisory.scenario.forbidden]
- [x] [serial] Export advisory counts in the review window and friction view with identity-stable encoding, and render them in the dashboard. r[kiln.observe.review.advisory]

## Phase 3: Verification

- [x] [serial] Property-style checklist test, eligible-vote equivalence, legacy byte-identity, shell parity tests over HTTPS, iroh and dashboard, a disposable-instance browser check, and operator documentation. r[kiln.review.advisory.scenario.checklist] r[kiln.review.advisory.scenario.votes] r[kiln.review.advisory.legacy] r[kiln.observe.review.advisory.scenario.parity]
