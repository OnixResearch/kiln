# Publication Recovery Specification

## Purpose

Defines the `publication-recovery` capability.

## Requirements

### Requirement: Exact durable publication effects

r[kiln.publication.recovery] Each external effect MUST reserve exact work/version/attempt/fence, operation and candidate ownership before transport. Unknown outcomes MUST reconcile before retry and observations MUST retain replay identities.

#### Scenario: Crash after transport

r[kiln.publication.recovery.scenario.crash]
- GIVEN a remote effect succeeded and its acknowledgement was lost
- WHEN the journal restarts
- THEN the caller MUST reconcile the same operation without blind duplicate creation

### Requirement: Exact remote evidence

r[kiln.publication.exact] Incomplete pagination, ambiguous matches and unexpected remote heads MUST NOT authorize overwrite or prove absence. External integration MUST match the admitted candidate and exact target mapping.

#### Scenario: External merge changed base

r[kiln.publication.exact.scenario.external]
- GIVEN a merged PR with an unexpected first parent
- WHEN its final target descends from that merge
- THEN integration MUST still be refused

### Requirement: Qualified atomicity and lifecycle binding

r[kiln.publication.atomicity] Unattended landing MUST preserve the admitted base atomically or remain unsupported and PR-ready. Cairn completion MUST bind the authoritative result to the exact outer work/task/spec/candidate/lifecycle policy. Source changes MUST already be reviewed in the candidate.

#### Scenario: Unsupported merge route

r[kiln.publication.atomicity.scenario.unsupported]
- GIVEN an ordinary head-guarded GitHub merge route
- WHEN unattended landing is requested
- THEN the route MUST refuse before mutation because a separate base read cannot prevent a base race
