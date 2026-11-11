# Aspen Proof Specification

## Purpose

Defines the `aspen-proof` capability.

## Requirements

### Requirement: Admitted Aspen profile construction proof

r[kiln.aspen.proof]
`AdmittedAspenRuntimeProfile` MUST be constructible and mutable only inside
`kiln-adapter-aspen`. External crates MUST read its profile and identity through
accessors.

#### Scenario: External construction is rejected
r[kiln.aspen.proof.scenario.construct]
- GIVEN an external crate importing `AdmittedAspenRuntimeProfile`
- WHEN it builds one with a struct literal
- THEN compilation fails because its fields are not public.

#### Scenario: Admitted facts remain readable
r[kiln.aspen.proof.scenario.read]
- GIVEN an admitted Aspen runtime profile
- WHEN a consumer asks for the profile or its identity
- THEN the corresponding borrowed accessor returns the admitted value.
