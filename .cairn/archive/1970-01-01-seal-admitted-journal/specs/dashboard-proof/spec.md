# Dashboard Proof Specification

## Purpose

Defines the construction boundary for admitted dashboard journals.

## ADDED Requirements

### Requirement: Admitted journal construction proof

r[kiln.observe.dashboard.proof]
`AdmittedJournal` MUST be constructible and mutable only inside
`kiln-dashboard`. External crates MUST read admitted state through accessors and
MUST attach a review window only through the named `with_review` operation.

#### Scenario: External construction is rejected
r[kiln.observe.dashboard.proof.scenario.construct]
- GIVEN an external crate importing `AdmittedJournal`
- WHEN it builds one with a struct literal
- THEN compilation fails because its fields are not public.

#### Scenario: Admitted facts remain readable
r[kiln.observe.dashboard.proof.scenario.read]
- GIVEN an admitted journal
- WHEN a consumer asks for its state, records, schema, declared identity or
  review window
- THEN the corresponding borrowed accessor returns the admitted value.
