# Radicle Mutation Specification

## Purpose

Defines the `radicle-mutation` capability.

## Requirements

### Requirement: Explicit Radicle mutation authority
r[kiln.radicle.shell_mutation] Collaboration mutation MUST use a fresh expected-head preflight and an explicitly resolved SecretSpec signer.

#### Scenario: Expected heads match
r[kiln.radicle.shell_mutation.scenario.authorized]
- GIVEN an authorized plan whose reloaded heads still match under single-writer rules
- WHEN the shell reloads, previews, and dispatches the mutation
- THEN it MAY sign and write the exact admitted operation.

#### Scenario: Signer is ambient or missing
r[kiln.radicle.shell_mutation.scenario.ambient]
- GIVEN a mutation plan without a resolvable SecretSpec signer declaration
- WHEN mutation preflight runs
- THEN Kiln MUST reject the effect before signing or writing
- AND no key material MAY appear in diagnostics, logs, or observations.

### Requirement: Declared single-writer targets
r[kiln.radicle.mutation_scope] Every mutation plan MUST name an admitted RID, an admitted object type, and one declared target identity, and every prior head MUST be authored by the resolved Kiln signer.

#### Scenario: Declared target with owned heads
r[kiln.radicle.mutation_scope.scenario.owned]
- GIVEN a plan inside all allowlists whose target heads are empty or authored by the resolved signer
- WHEN preflight runs
- THEN the shell MAY proceed to preview and dispatch.

#### Scenario: Undeclared target or foreign head
r[kiln.radicle.mutation_scope.scenario.foreign]
- GIVEN a plan outside any allowlist, or a head authored by another identity
- WHEN preflight runs
- THEN Kiln MUST reject the plan before signing.

### Requirement: Bounded mutation phases
r[kiln.radicle.mutation_bounds] Preflight, signing, and write phases MUST run under named deadlines, and ambiguous outcomes MUST remain unknown until reconciliation.

#### Scenario: Deadline expires mid-write
r[kiln.radicle.mutation_bounds.scenario.timeout]
- GIVEN a write that outlives its named deadline after possible acceptance
- WHEN outcome classification runs
- THEN Kiln MUST record timeout or unknown as classified
- AND it MUST NOT redispatch without reconciliation evidence.

#### Scenario: Reconciliation confirms a recorded write
r[kiln.radicle.mutation_bounds.scenario.reconcile]
- GIVEN an unknown mutation whose operation identity is observable in storage
- WHEN reconciliation runs
- THEN Kiln MUST adopt the recorded observation exactly once.

### Requirement: Radicle mutation conformance
r[kiln.radicle.mutation_conformance] Maintained conformance MUST include native fixtures created by the pinned cohort covering authorized writes plus moved-head, foreign-head, undeclared-target, missing-signer, oversized-payload, redaction, timeout, and reconciliation cases.

#### Scenario: Authorized update repeats identically
r[kiln.radicle.mutation_conformance.scenario.repeat]
- GIVEN one stable local fixture repository with a generated fixture key
- WHEN the adapter dispatches one operation identity more than once
- THEN each observation MUST stay equal and repeatable.
