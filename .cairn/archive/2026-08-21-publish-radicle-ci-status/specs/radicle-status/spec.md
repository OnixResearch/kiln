# Radicle CI Status Delta Specification

## ADDED Requirements

### Requirement: Radicle status publication
r[kiln.radicle.shell_status] Radicle CI status publication MUST remain an idempotent rebuildable projection without canonical CI state authority.

#### Scenario: Status write succeeds
r[kiln.radicle.shell_status.scenario.success]
- GIVEN a status effect bound to one exact attempt and idempotency key
- WHEN the provider records the status in the declared ci-job object
- THEN Kiln MUST retain the native reference as a provider observation.

#### Scenario: Connection fails after acceptance
r[kiln.radicle.shell_status.scenario.unknown]
- GIVEN a status request accepted before the write outcome becomes clear
- WHEN outcome classification runs
- THEN Kiln MUST record an unknown outcome and require reconciliation.

### Requirement: SecretSpec-resolved status signer
r[kiln.radicle.status_signer] The status signer MUST resolve through one explicit SecretSpec declaration and MUST fail closed before signing when the declaration is missing or unresolved.

#### Scenario: Declared signer resolves
r[kiln.radicle.status_signer.scenario.resolved]
- GIVEN a profile whose SecretSpec declaration resolves to a usable key
- WHEN the shell dispatches an admitted status plan
- THEN it MAY sign and write with that resolved identity.

#### Scenario: Signer declaration is absent
r[kiln.radicle.status_signer.scenario.missing]
- GIVEN a status plan whose SecretSpec declaration is missing or unresolved
- WHEN dispatch preflight runs
- THEN Kiln MUST deny the effect before signing
- AND no key material MAY appear in diagnostics, logs, or observations.

### Requirement: Bounded status target allowlists
r[kiln.radicle.status_allowlist] Each status plan MUST name an admitted RID and the `xyz.radicle.ci-job` object type, and the single-writer preflight MUST reject foreign heads.

#### Scenario: Target is admitted and heads are single-writer
r[kiln.radicle.status_allowlist.scenario.admitted]
- GIVEN a plan for an allowed RID whose current head is absent or authored by the resolved signer
- WHEN preflight runs
- THEN the shell MAY dispatch the exact planned update.

#### Scenario: Target or head is foreign
r[kiln.radicle.status_allowlist.scenario.foreign]
- GIVEN a plan outside the RID or type allowlists, or a current head Kiln did not author
- WHEN preflight runs
- THEN Kiln MUST reject the plan before signing or writing.

### Requirement: Radicle status conformance
r[kiln.radicle.status_conformance] Maintained conformance MUST include native fixtures created by the pinned cohort plus redaction, drift, and reconciliation cases.

#### Scenario: Native fixture resolves repeatedly
r[kiln.radicle.status_conformance.scenario.repeat]
- GIVEN one stable local repository fixture with a generated fixture key
- WHEN the adapter writes and rewrites one idempotency key more than once
- THEN each observation MUST stay equal and repeatable.

#### Scenario: Secret material leaks
r[kiln.radicle.status_conformance.scenario.redaction]
- GIVEN any failure path after secret resolution
- WHEN Kiln renders diagnostics, errors, or logs
- THEN resolved key bytes MUST NOT appear.
