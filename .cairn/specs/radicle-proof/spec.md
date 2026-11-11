# Radicle Proof Specification

## Purpose

Defines the `radicle-proof` capability.

## Requirements

### Requirement: Admitted Radicle config construction proof

r[kiln.radicle.proof]
`AdmittedRadicleConfig` MUST be constructible and mutable only inside
`kiln-adapter-radicle`. External crates MUST read its shell, Lattice and
transport through accessors.

#### Scenario: External construction is rejected
r[kiln.radicle.proof.scenario.construct]
- GIVEN an external crate importing `AdmittedRadicleConfig`
- WHEN it builds one with a struct literal
- THEN compilation fails because its fields are not public.

#### Scenario: Admitted facts remain readable
r[kiln.radicle.proof.scenario.read]
- GIVEN an admitted Radicle config
- WHEN a consumer asks for its shell, Lattice or transport profile
- THEN the corresponding borrowed accessor returns the admitted value.
