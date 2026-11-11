# Github Proof Specification

## Purpose

Defines the `github-proof` capability.

## Requirements

### Requirement: Admitted GitHub config construction proof

r[kiln.github.profile.proof]
`AdmittedGithubConfig` MUST be constructible and mutable only inside
`kiln-adapter-github`. External crates MUST read its config, identities, Lattice
and transport through accessors.

#### Scenario: External construction is rejected
r[kiln.github.profile.proof.scenario.construct]
- GIVEN an external crate importing `AdmittedGithubConfig`
- WHEN it builds one with a struct literal
- THEN compilation fails because its fields are not public.

#### Scenario: Admitted facts remain readable
r[kiln.github.profile.proof.scenario.read]
- GIVEN an admitted GitHub config
- WHEN a consumer asks for its config, identities, Lattice or transport
- THEN the corresponding borrowed accessor returns the admitted value.
