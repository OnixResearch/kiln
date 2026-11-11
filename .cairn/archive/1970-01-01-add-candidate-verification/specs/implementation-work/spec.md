# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Canonical candidate verification

r[kiln.implementation.verification] Kiln MUST admit candidate verification only
from a retained canonical CI fact bound to the exact candidate handoff,
work/version, attempt/fence, candidate and tree, base and verification profile.
The run MUST be terminal, successful, clean, full and trusted, use the exact
candidate revision, and expose nonempty unique checks that all pass. Admission
MUST NOT execute checks or create CI facts. Repeating an equal receipt MUST be
idempotent; partial, failed, stale, differently bound or untrusted evidence MUST
be refused. A recorded receipt MUST move the attempt to the terminal `verified`
state and preserve the receipt for audit.

#### Scenario: Exact canonical evidence verifies the candidate

r[kiln.implementation.verification.scenario.admit]
- GIVEN an attempt with an admitted candidate handoff
- WHEN retained canonical CI evidence passes the exact binding and check requirements
- THEN the attempt MUST become `verified` and retain the receipt idempotently

#### Scenario: Partial, failed or mismatched evidence is refused

r[kiln.implementation.verification.scenario.refused]
- GIVEN failed, incomplete, untrusted, empty-check, stale-revision or differently bound evidence
- WHEN verification admission runs
- THEN it MUST fail without changing the attempt state
