# Implementation qualification

## ADDED Requirements

### Requirement: Explicit real-stage qualification

r[kiln.implementation_qualification.real_stages]

The runner MUST compose actual bounded implementation and independent verification consumers using explicit immutable program, source, policy, authority and provider inputs. Missing prerequisites MUST fail closed.

#### Scenario: Missing provider

- GIVEN no admitted worker provider
- WHEN a live case is selected
- THEN qualification reports the exact missing prerequisite and does not manufacture worker evidence.

### Requirement: Scoped evidence

r[kiln.implementation_qualification.scoped_evidence]

The runner MUST retain per-scenario raw outputs and exact identities and distinguish local scripted fixtures, actual worker evidence, verified PR, landing and source reconciliation.

#### Scenario: Frozen bug

- GIVEN an immutable baseline and reproducer
- WHEN the baseline fails and candidate succeeds under independent checks
- THEN the receipt retains both actual executions without treating an empty successful worker run as acceptance.

### Requirement: Remote approval

r[kiln.implementation_qualification.approval]

Remote publication, integration and source closure MUST require exact point-of-risk operator approval and MUST NOT execute as part of local qualification.

#### Scenario: Local run

- GIVEN local qualification authority only
- WHEN publication is unavailable
- THEN PR, landing and source reconciliation remain explicitly unqualified.
