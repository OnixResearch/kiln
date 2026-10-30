# Github Event Proof Specification

## Purpose

Defines the `github-event-proof` capability.

## Requirements

### Requirement: VerifiedEvent is a construction proof

r[kiln.github.event_proof]
`VerifiedEvent` MUST be constructible only by the admitted-event functions
(`admit_event` and `admit_event_with`) inside the `events` module. Its fields MUST
be private and readable only through getters. It MUST NOT implement `Serialize`
or `Deserialize`, because no production path serializes an admitted event and a
proof type MUST NOT be reconstructible from JSON. Signature verification, replay
protection, body bounds, selection and emitted wire behaviour MUST remain
unchanged.

#### Scenario: Outside code cannot forge an admitted event

r[kiln.github.event_proof.scenario.forgery]
- GIVEN a crate that depends on `kiln-adapter-github`
- WHEN it builds a `VerifiedEvent` with a struct literal or deserializes one from
  JSON
- THEN both MUST fail to compile, AND every readable fact MUST come from a getter
  on a value produced by `admit_event`

### Requirement: Admitted proof-type audit

r[kiln.github.event_proof.audit]
The change record MUST classify each remaining `Admitted*` type as a proof (to be
sealed in a separate change) or a plain record, with one line per type, and MUST
NOT change those types here.

#### Scenario: Audit naming

r[kiln.github.event_proof.audit.scenario.names]
- GIVEN the audit record
- WHEN a reviewer reads it
- THEN `AdmittedGithubConfig`, `AdmittedRadicleConfig`,
  `AdmittedAspenRuntimeProfile` and `AdmittedJournal` MUST each be named as proofs
  to seal separately, AND `AdmittedResult` MUST be named as a plain record with
  its reason
