## Phase 1: Core

- [x] [serial] Add `Authorship`, `AuthorshipClass`, validation, the declared-only class fold and the ownership path lookup to `kiln-core::change_review`, and store authorship on `Revision` with legacy-compatible serde. r[kiln.review.authorship] r[kiln.review.authorship.scenario.legacy] r[kiln.review.authorship.scenario.invalid]
- [x] [serial] Add review bounds, review view kinds, fields, requests, bodies and window to `kiln-observe-core`, including window admission, the pure state-to-window derivation, projection and identity framing. r[kiln.observe.review.window] r[kiln.observe.review.window.scenario.violation] r[kiln.observe.review.friction] r[kiln.observe.review.anchors] r[kiln.observe.review.anchors.scenario.paging]
- [x] [serial] Add the review journal and window fixtures, and update the observation Nickel profile and its generated fixtures. r[kiln.observe.review.friction.scenario.fixture]

## Phase 2: Shells and surfaces

- [x] [serial] Accept, validate and journal authorship on web create, contribute and upload, show it on change detail, export the review window, and add the console select. r[kiln.review.authorship.scenario.declared] r[kiln.review.authorship.console]
- [x] [serial] Serve review views over the HTTPS and iroh shells, and render them in the offline dashboard. r[kiln.observe.review.parity] r[kiln.observe.review.parity.scenario.equal]
- [x] [serial] Document self-declared, unverified authorship and the non-authoritative views. r[kiln.observe.review.nonclaims]

## Phase 3: Verification

- [x] [serial] Run focused fmt, clippy and tests for the touched crates, the console node tests and the Nickel profile comparisons, then record evidence. r[kiln.review.authorship] r[kiln.observe.review.friction] r[kiln.observe.review.parity]

Accepted on operator request 2026-10-30 after parent integration review; see `evidence/verification.json`.
