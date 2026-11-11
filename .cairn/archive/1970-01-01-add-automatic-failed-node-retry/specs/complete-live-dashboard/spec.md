# Complete Live Dashboard Specification

## ADDED Requirements

### Requirement: Automatic bounded failed-node retry

r[kiln.web.retry_failed] A project MAY declare `retry_failed` in the range 0–3,
default 0. When a run reaches `DomainFailure`, `Denied`, `TimedOut` or
`ProviderError`, the service MUST issue the existing failed-node rerun on the
exact same source while the run's automatic retry ordinal is below
`retry_failed`, with a deterministic request identity derived from the parent
run and the next ordinal. The retry child MUST keep the parent link and the
parent's untrusted gate, and the parent outcome MUST remain unchanged.

#### Scenario: A passing retry is visible without turning the parent green

r[kiln.web.retry_failed.scenario.pass]
- GIVEN a project with `retry_failed` greater than zero and a run that fails
- WHEN a later partial-node retry passes
- THEN the parent MUST retain its failed outcome
- AND the run view MUST show the passing retry child
- AND the partial-node child MUST NOT satisfy required CI

#### Scenario: The retry chain stops at the reviewed bound

r[kiln.web.retry_failed.scenario.bound]
- GIVEN a project with `retry_failed = N`
- WHEN every retry also fails
- THEN exactly N automatic retry children MUST be created
- AND no further retry MUST be issued

#### Scenario: Disabled or refused retries create nothing

r[kiln.web.retry_failed.scenario.refused]
- GIVEN `retry_failed = 0`, an unknown node state, an unresolved dirty snapshot, a superseded untrusted run, or an exhausted request/run retention bound
- WHEN a failed run becomes terminal
- THEN no automatic retry MUST be created
- AND the failed outcome and retained state MUST remain unchanged

#### Scenario: Disabled by default

r[kiln.web.retry_failed.scenario.disabled]
- GIVEN a project without `retry_failed`
- WHEN a run fails
- THEN the service MUST behave exactly as before automatic retry existed
