# Radicle Mutation Specification

## Purpose

Defines the `radicle-mutation` capability.

## Requirements

### Requirement: Explicit Radicle mutation authority

r[kiln.radicle.shell_mutation] Collaboration mutation MUST use a fresh authenticated expected-head preflight, an explicitly resolved SecretSpec signer, and expected-old own-namespace publication. Native upstream permissions determine which signed participants may contribute; Kiln allowlists do not grant merge authority.

#### Scenario: Expected heads match

r[kiln.radicle.shell_mutation.scenario.authorized]
- GIVEN an authorized plan whose authenticated head set still matches
- WHEN the shell resolves its declared signer, previews, and dispatches
- THEN it MAY sign the exact admitted operation and publish only if the observed own namespace is unchanged.

#### Scenario: Signer is ambient or missing

r[kiln.radicle.shell_mutation.scenario.ambient]
- GIVEN a mutation plan without a resolvable SecretSpec signer declaration
- WHEN mutation preflight runs
- THEN Kiln MUST reject the effect before signing or writing
- AND no key material MAY appear in diagnostics, logs, or observations.

### Requirement: Declared upstream-authorized targets

r[kiln.radicle.mutation_scope] Every mutation plan MUST name an admitted RID, object type, and declared target identity. Every advertised head MUST have authenticated namespace signed refs, a valid change signature, a path to the declared root, bounded contents, and a recognized native action encoding. Prior heads need not be authored by the Kiln signer; pinned upstream actor permissions remain authoritative.

#### Scenario: Declared target with authenticated heads

r[kiln.radicle.mutation_scope.scenario.owned]
- GIVEN a plan inside all allowlists with an authenticated root-connected head set
- WHEN native preflight succeeds
- THEN the shell MAY proceed to preview and dispatch without discarding another authorized writer's history.

#### Scenario: Undeclared target or unauthenticated head

r[kiln.radicle.mutation_scope.scenario.foreign]
- GIVEN an unlisted target, substituted namespace mapping, invalid signature, disconnected head, or unknown action encoding
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

r[kiln.radicle.mutation_conformance] Maintained conformance MUST include pinned native fixtures covering authorized multi-writer writes, moved heads, invalid namespace mappings, undeclared targets, missing signers, oversized payloads, redaction, timeout, and reconciliation. Recognized but unauthorized actions MUST follow upstream permission semantics rather than being mistaken for unknown encodings.

#### Scenario: Authorized update repeats identically

r[kiln.radicle.mutation_conformance.scenario.repeat]
- GIVEN one stable local fixture repository with a generated fixture key
- WHEN the adapter dispatches one operation identity more than once
- THEN each observation MUST stay equal and repeatable without a duplicate review.
