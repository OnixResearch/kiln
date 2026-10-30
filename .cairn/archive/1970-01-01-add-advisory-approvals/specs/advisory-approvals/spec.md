## ADDED Requirements

### Requirement: Advisory approval command

r[kiln.review.advisory]
The review domain MUST provide separate advisory approve and withdraw commands that are not vote values. An advisory approval MUST be recorded as `{ actor, revision, created_ms }` in a collection separate from votes and MUST bind the exact current revision. Any principal with read access to the change MAY cast or withdraw one; the change owner and revision authors MUST be rejected, and closed changes and stale revisions MUST be rejected. Uploading a new revision MUST make prior advisory approvals historical.

#### Scenario: Cast, withdraw and historical

r[kiln.review.advisory.scenario.lifecycle]
- GIVEN an open change and a read principal who is neither its owner nor an author
- WHEN the principal casts an advisory approval, then a new revision is uploaded
- THEN the approval MUST be listed for the earlier revision only, MUST be counted as historical, and withdrawing on the new revision MUST fail as missing

#### Scenario: Owner and author rejected

r[kiln.review.advisory.scenario.forbidden]
- GIVEN a change
- WHEN its owner or a revision author casts an advisory approval, or a read-only principal sends any other command
- THEN the domain MUST reject the advisory command and the shell MUST deny the other command

### Requirement: Advisory approvals carry no authority

r[kiln.review.advisory.authority]
Advisory approvals MUST NOT satisfy required approvals, ownership, veto clearing, CI or queue admission, MUST NOT clear, create or transfer to a candidate, MUST NOT add attention, notifications or delivery recipients, and MUST grant no write capability. Eligible reviewers' votes MUST behave exactly as before.

#### Scenario: No checklist item flips

r[kiln.review.advisory.scenario.checklist]
- GIVEN every combination of checklist-relevant review state in the property test
- WHEN advisory approvals are added by every principal allowed to advise
- THEN every checklist item and the ready flag MUST be identical to the state without advisory approvals

#### Scenario: Eligible votes unchanged

r[kiln.review.advisory.scenario.votes]
- GIVEN eligible reviewer votes with and without advisory approvals present
- WHEN votes are cast, replaced and counted
- THEN vote admission, stored votes and checklist results MUST be identical

### Requirement: Advisory visibility and search

r[kiln.review.advisory.view]
The change detail MUST show advisory approvals in a separate list labelled non-binding, distinguishing current from historical, and search MUST offer an advisory filter that saved views can store.

### Requirement: Advisory counts in observation views

r[kiln.observe.review.advisory]
The review-window export MUST carry per-change advisory approval counts for the latest admitted patch set, bounded by a named limit, and the review-friction view MUST report advisory totals per authorship class. The HTTPS shell, the iroh shell and the dashboard MUST render them identically.

#### Scenario: Shell parity

r[kiln.observe.review.advisory.scenario.parity]
- GIVEN a review window with advisory approvals
- WHEN the review-friction view is requested over HTTPS, over iroh and rendered by the dashboard
- THEN each MUST equal the direct projection, including advisory counts and view identity

### Requirement: Legacy replay

r[kiln.review.advisory.legacy]
Journals, saved views, review windows and view identities written before advisory approvals MUST deserialize and re-serialize byte-identically, and their identities MUST be unchanged.
