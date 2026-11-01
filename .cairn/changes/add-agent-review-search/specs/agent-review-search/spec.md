# Agent Review Search Specification

## Purpose

Defines a read-only review search filter for attested agent submissions.

## Requirements

### Requirement: Current-revision agent filter

r[kiln.review.agent.search] `SearchQuery` MAY carry `agent: Option<bool>`.
`Some(true)` MUST match only changes whose current revision carries
`submitted_by`; `Some(false)` MUST match only changes without it; absence MUST
not filter. The field MUST be omitted when absent so existing saved-view bytes
remain unchanged. The shell MUST accept only `agent=with` or `agent=without`.

#### Scenario: Attested change matches
r[kiln.review.agent.search.scenario.attested]
- GIVEN a change whose current revision is attested agent-submitted
- WHEN searching with `agent=with`
- THEN the change is returned.

#### Scenario: Unattested change excluded
r[kiln.review.agent.search.scenario.unattested]
- GIVEN a change without current-revision attestation
- WHEN searching with `agent=with`
- THEN the change is excluded.

### Requirement: Filter remains read-only

r[kiln.review.agent.search.boundary] The filter MUST NOT alter attestation,
authorship, review state, approvals, candidate preparation or merge readiness.
It MUST be available in the console query and saved views.

#### Scenario: Saved view
r[kiln.review.agent.search.scenario.saved]
- GIVEN a saved view with an agent filter
- WHEN the view is applied
- THEN the same read-only filter is restored.
