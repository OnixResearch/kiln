# Exploration Cases Delta

## ADDED Requirements

### Requirement: Stable exploration case IDs
r[kiln.web.exploration.cases] Each named exploration case MUST expose a stable case_id, a canonical recipe object with id, version and BLAKE3 revision, source revision, declared presentation and interaction requirements, and resolved subjects. Plain examples MUST NOT receive fabricated case identities. Runtime IDs MUST remain distinct per workspace.

#### Scenario: Two clean starts
- GIVEN two freshly seeded workspaces
- WHEN the same case_id is requested
- THEN semantic state and recipe identity match while runtime IDs differ.

#### Scenario: Unknown or moved case
- GIVEN an unknown case_id, moved recipe or cross-workspace reference
- WHEN resolution is attempted
- THEN it fails explicitly without title-based fallback.

### Requirement: Server-side case resolution
r[kiln.web.exploration.cases.resolution] GET /examples/cases/<case_id> MUST return workspace, case, recipe, subjects and route. Unknown case MUST return 404, recipe mismatch 409 and foreign workspace 409.

### Requirement: Comparable UI capture
r[kiln.web.exploration.cases.capture] Capture MUST drive a supplied Chromium executable over the DevTools protocol with a disposable profile, record the browser version in its provenance manifest, retain a screenshot and a bounded semantic/accessibility observation under named limits, and assert readiness before capture. Incompatible settings, including a different browser version, MUST be reported; subject ID changes MUST never be normalized away.

#### Scenario: Readiness failure
- GIVEN a wrong revision, unfinished loading state or missing resource
- WHEN capture is attempted
- THEN it is not reported as a valid matching observation.

### Requirement: Bounded synthetic exploration
r[kiln.web.exploration.cases.synthetic] The Bombadil rail MUST remain read-only and synthetic execution MUST remain visibly synthetic. Mutation cases, if any, MUST run only against owned disposable examples.

#### Scenario: Blocked migration cohort
- GIVEN dependency on unavailable Rats #16 and Meteorite build (#67/#68)
- WHEN the migration cohort is considered
- THEN it is recorded as blocked rather than simulated.
