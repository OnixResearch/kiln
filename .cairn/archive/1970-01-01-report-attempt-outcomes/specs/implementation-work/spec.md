# Implementation Work Specification

## ADDED Requirements

### Requirement: Attempt outcome and model-request observations

r[kiln.implementation.outcome_metrics] Kiln MUST preserve observed worker
terminal state separately from controller verification and optional model-request
usage for an exact attempt/fence. Complete, incomplete and absent usage MUST
remain distinct. Metrics MUST NOT grant admission, verification or publication.
Opt-in reports MUST retain counts, denominators and coverage for a selected
attempt window without ranking profiles on small sample gaps.

#### Scenario: Supported and unsupported usage

r[kiln.implementation.outcome_metrics.scenario.usage]
- GIVEN a bound worker observation with known model requests or unsupported telemetry
- WHEN usage is projected
- THEN supported counts MUST retain their exact number and coverage
- AND missing telemetry MUST remain absent with a reason, never JSONL events or zero

#### Scenario: Bound durable outcomes

r[kiln.implementation.outcome_metrics.scenario.binding]
- GIVEN Completed/verified, Completed/rejected, Failed, Aborted and deadline observations
- WHEN retained and replayed across restart
- THEN terminal and verification observations MUST remain separately available
- AND stale fences or conflicting observations MUST be refused

#### Scenario: Windowed report

r[kiln.implementation.outcome_metrics.scenario.report]
- GIVEN finished attempts with known, missing and incomplete telemetry
- WHEN an operator selects a half-open claim-time window
- THEN the report MUST show rejected Completed shares, disjoint failure counts,
  turns and wall time per verified pass, failed-turn share, sample coverage and
  deterministic profile/model breakdowns
- AND unknown denominators MUST remain absent and arithmetic overflow MUST fail

#### Scenario: Observation is not authority

r[kiln.implementation.outcome_metrics.scenario.authority]
- GIVEN a worker reports Completed with metrics or missing telemetry
- WHEN metrics are recorded or reported
- THEN existing verification, checklist, publication and attempt admission MUST be unchanged
- AND documentation MUST state that subscription cost is turns, tokens and wall
  time, not an inferred dollar charge
