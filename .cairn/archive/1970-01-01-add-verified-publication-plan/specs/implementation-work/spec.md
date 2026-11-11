# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Verified publication preparation

r[kiln.implementation.publication] Kiln MUST prepare verified-PR delivery only
for a terminal `verified` attempt with the retained candidate handoff and
verification receipt. The plan MUST bind the exact work/version, attempt/fence,
repository, frozen base, candidate and tree, deterministic work/attempt branch,
and bounded title/body. The work MUST select verified-pull-request delivery.
Preparation MUST NOT perform a remote effect. Observed branch and pull-request
records MUST preserve phase order and the exact verified candidate head;
different or out-of-order observations MUST be refused.

#### Scenario: Prepare a verified candidate

r[kiln.implementation.publication.scenario.prepare]
- GIVEN a verified attempt with retained handoff and verification
- WHEN an exact publication plan is prepared
- THEN it MUST bind the deterministic branch and exact candidate
- AND preparing MUST remain a local state transition

#### Scenario: Unverified or arbitrary publication is refused

r[kiln.implementation.publication.scenario.refused]
- GIVEN an unverified attempt or an arbitrary branch/title/base/candidate binding
- WHEN publication preparation runs
- THEN it MUST fail without a remote effect
