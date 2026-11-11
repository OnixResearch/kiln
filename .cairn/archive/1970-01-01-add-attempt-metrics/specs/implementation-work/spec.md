# Implementation Work Specification

## ADDED Requirements

### Requirement: Opt-in attempt metrics receipt

r[kiln.implementation.metrics] Kiln MAY produce a default-off metrics receipt
for one claimed attempt binding the attempt/fence/work identity and revision,
claim and terminal times, elapsed milliseconds, open/terminal phase, optional
trace references, and token and auxiliary-call observations. Missing cost data
MUST be represented as absent with a reason, never zero. Observation MUST NOT
gate admission, verification or publication. Only an explicit record request
MUST retain the first receipt on the attempt; repeated recording MUST NOT
overwrite it. Stale fences, traces over the bound, duplicate traces and
observations before the claim MUST fail.

#### Scenario: Open attempt

r[kiln.implementation.metrics.scenario.open]
- GIVEN a claimed open attempt
- WHEN metrics are observed at a later instant
- THEN elapsed time MUST span claim to observation and the receipt MUST be open
- AND absent usage MUST stay absent

#### Scenario: Terminal attempt

r[kiln.implementation.metrics.scenario.terminal]
- GIVEN a cancelled, failed or blocked attempt with a recorded end
- WHEN metrics are recorded
- THEN elapsed time MUST span claim to end and the receipt MUST be closed

#### Scenario: Default off

r[kiln.implementation.metrics.scenario.default_off]
- GIVEN an attempt with no record request
- WHEN metrics are observed
- THEN no receipt MUST be retained
- AND no admission, verification or publication decision may change

#### Scenario: First receipt is retained

r[kiln.implementation.metrics.scenario.retained]
- GIVEN an attempt with a retained metrics receipt
- WHEN recording is repeated
- THEN the original receipt MUST be returned unchanged

#### Scenario: Invalid observation is refused

r[kiln.implementation.metrics.scenario.binding]
- GIVEN a stale fence, duplicate traces, too many traces or a time before the claim
- WHEN metrics are observed
- THEN the observation MUST fail
