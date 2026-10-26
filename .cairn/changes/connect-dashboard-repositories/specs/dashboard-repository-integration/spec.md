# Dashboard Repository Integration Specification

## Purpose

Connect the offline read-only dashboard to real CI records without changing provider authority or canonical state.

## Requirements

### Requirement: Canonical GitHub observation export
r[kiln.dashboard.repositories.github_export]
The GitHub shell MUST export an existing exact run as canonical journal records over its persisted events, verify event identity framing and replay, open the existing replay database read only, and perform no schema mutation, credential resolution, dispatch or status publication.

#### Scenario: Valid and rejected run export
r[kiln.dashboard.repositories.github_export.scenario.admission]
- GIVEN an existing profile-bound GitHub run or a missing, malformed, stale-profile or corrupt store
- WHEN observation export is requested
- THEN a valid run MUST preserve its canonical run/attempt outcome and deterministic event order, and invalid inputs MUST fail without changing the database or emitting a successful report.

### Requirement: Bounded repository and history catalog
r[kiln.dashboard.repositories.catalog]
The dashboard MUST admit a closed versioned catalog with named byte/project/history bounds, deterministic path resolution and unique project/history identifiers, independently admit every history through the existing journal/store contracts, and render offline repository/history navigation without merging revisions or canonical identities. Declared repository/source labels MUST remain distinguishable from verified CI facts.

#### Scenario: Navigation, reload and no script
r[kiln.dashboard.repositories.catalog.scenario.navigation]
- GIVEN multiple independently admitted histories and optional colliding run identifiers
- WHEN the operator switches repository/history, reloads a watched report, disables JavaScript or selects a removed history
- THEN each report MUST retain its own identities and redactions, valid selections MUST survive reload, no-script readers MUST access the histories, and removed selections MUST be reported without silently substituting another history.

### Requirement: Atomic report delivery
r[kiln.dashboard.repositories.delivery]
A separately configured imperative shell MUST generate the actual admitted HTML report and atomically deliver only its bounded bytes to the explicit local or SSH target, with finite process deadlines, no shell-interpolated untrusted inputs, and no credential or canonical database transfer.

#### Scenario: Generation or transfer failure
r[kiln.dashboard.repositories.delivery.scenario.failure]
- GIVEN a previously delivered valid report
- WHEN generation fails, the report exceeds a named bound, or transfer does not complete
- THEN the previous report MUST remain readable, the shell MUST fail visibly, and it MUST NOT publish partial bytes or manufacture a new successful observation.

### Requirement: Real repository evidence
r[kiln.dashboard.repositories.real_ci]
The integration MUST use actual repository source and real CI observations, distinguish fresh execution from retained runs, preserve exact revision and unresolved outcomes, and not claim publication, rollout, source trust or newest-history completeness.

#### Scenario: Evidence and authority scope
r[kiln.dashboard.repositories.real_ci.scenario.scope]
- GIVEN real passing/failing records, duplicate/restart observations or unavailable live prerequisites
- WHEN integration verification is recorded
- THEN it MUST name the actual commands and outputs exercised, preserve unresolved states, identify retained versus fresh evidence, and state unavailable authority/runtime prerequisites without substituting a synthetic live claim.
