# Observation Projection Delta Specification

## ADDED Requirements

### Requirement: Bounded observation profile
r[kiln.observe.profile] Kiln MUST define typed observation profiles that declare the supported view vocabulary, exact named bounds, paging policy, cursor windows, redaction policy, and non-claims.

#### Scenario: Profile matches declared views
r[kiln.observe.profile.scenario.match]
- GIVEN a complete observation profile over the supported view vocabulary
- WHEN observation admission runs
- THEN Kiln MAY serve the declared views under their declared bounds.

#### Scenario: View is outside the vocabulary
r[kiln.observe.profile.scenario.unknown_view]
- GIVEN a view request whose kind is not in the profile vocabulary
- WHEN observation admission runs
- THEN Kiln MUST reject it before projection.

### Requirement: Deterministic read-only projection
r[kiln.observe.projection] Equal admitted state, profile facts, and window facts MUST produce equal observation views, and projection MUST NOT mutate state or request effects.

#### Scenario: Equal inputs produce equal views
r[kiln.observe.projection.scenario.equal]
- GIVEN equal admitted state, profile, and window inputs
- WHEN two observation projections run
- THEN Kiln MUST produce equal views.

#### Scenario: Projection is read-only
r[kiln.observe.projection.scenario.read_only]
- GIVEN one committed state and one projection run
- WHEN the projection completes
- THEN the source state MUST remain unchanged and no effect request MUST be emitted.

### Requirement: Registered rebuildable view identity
r[kiln.observe.identity] Every observation view MUST bind an exact `kiln.observe.view.v1` BLAKE3 identity, the source state revision, the view request, and the admitted window.

#### Scenario: State revision is unknown
r[kiln.observe.identity.scenario.unknown_state]
- GIVEN a request that names a state revision the store does not hold
- WHEN observation admission runs
- THEN Kiln MUST reject the request before projection.

#### Scenario: View rebuilds to the same identity
r[kiln.observe.identity.scenario.rebuild]
- GIVEN the same committed history and the same view request
- WHEN the view is rebuilt from the journal
- THEN the rebuilt view MUST equal the previously projected view and identity.

### Requirement: Bounded observation admission
r[kiln.observe.safety] Observation projection MUST enforce declared page, window, attribute, value, and target bounds, and MUST redact undeclared fields before a view is returned.

#### Scenario: Target does not exist
r[kiln.observe.safety.scenario.unknown_target]
- GIVEN a run, attempt, or log stream identifier that is not in the admitted state or store
- WHEN the view projection runs
- THEN Kiln MUST return a bounded not-found view or reject the request without claiming the target exists.

#### Scenario: Page exceeds the bound
r[kiln.observe.safety.scenario.page_bound]
- GIVEN a paged view whose requested limit exceeds the profile page bound
- WHEN projection admission runs
- THEN Kiln MUST cap the page to the declared bound and report the truncation.

#### Scenario: Log cursor is outside the retained window
r[kiln.observe.safety.scenario.cursor_window]
- GIVEN a log-range request whose cursor or window lies outside the retained stream range
- WHEN the log window is admitted
- THEN Kiln MUST report the exact retained window instead of claiming bytes that were not retained.

#### Scenario: Field is undeclared or secret
r[kiln.observe.safety.scenario.redaction]
- GIVEN a field classified as secret or outside the profile attribute set
- WHEN redaction admission runs
- THEN Kiln MUST redact or omit that field before the view is encoded.

### Requirement: Maintained observation conformance
r[kiln.observe.conformance] Maintained conformance MUST cover every supported view with deterministic, paging, window, identity, redaction, unknown-target, and rebuild fixtures, including negative fixtures for every failure policy.

#### Scenario: Negative fixture is missing
r[kiln.observe.conformance.scenario.negative_missing]
- GIVEN an observation candidate whose fixture set lacks a required negative case
- WHEN conformance runs
- THEN the candidate MUST fail.

### Requirement: Observation projection non-claims
r[kiln.observe.nonclaims] Passing observation checks MUST NOT claim transport delivery, live freshness, control execution, provider correctness, CI success, or release readiness.

#### Scenario: A view passes
r[kiln.observe.nonclaims.scenario.scope]
- GIVEN a passing observation fixture
- WHEN Kiln records evidence
- THEN the evidence MUST remain scoped to the checked projection, profile, window, bounds, and view identity.
