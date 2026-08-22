# Radicle Source Contract Specification

## Purpose

Defines the `radicle-source-contract` capability.

## Requirements

### Requirement: Runtime-independent source contract
r[kiln.radicle.source_contract.boundary] Kiln MUST publish the Radicle source contract without Git, Radicle runtime, process, network, filesystem, clock, storage, or credential dependencies.

#### Scenario: Standalone consumer builds the contract
r[kiln.radicle.source_contract.boundary.scenario.standalone]
- GIVEN a consumer with only `core` and `alloc`
- WHEN the consumer builds the contract for `wasm32-unknown-unknown`
- THEN the contract MUST build without provider runtime dependencies.

#### Scenario: Runtime type enters the contract graph
r[kiln.radicle.source_contract.boundary.scenario.runtime]
- GIVEN a contract candidate whose normal dependency graph contains a Git or Radicle runtime crate
- WHEN dependency admission runs
- THEN the candidate MUST be rejected.

### Requirement: Exact signed-reference request
r[kiln.radicle.source_contract.request] A source request MUST bind one exact RID, signer namespace, branch reference, expected signed-refs commit, and expected revision.

#### Scenario: Request declares all exact identities
r[kiln.radicle.source_contract.request.scenario.exact]
- GIVEN a versioned request with non-empty exact identities and positive bounds
- WHEN request admission runs
- THEN the request MUST be accepted.

#### Scenario: Request uses a floating signed-reference tip
r[kiln.radicle.source_contract.request.scenario.floating]
- GIVEN a request without an expected signed-refs commit
- WHEN request admission runs
- THEN the request MUST be rejected before provider effects.

### Requirement: Parent-level signed-reference observation
r[kiln.radicle.source_contract.sigrefs] An accepted observation MUST bind a verified signer, RID identity root, branch target, and stable signed-refs commit at feature level `parent`.

#### Scenario: Parent-level observation is stable
r[kiln.radicle.source_contract.sigrefs.scenario.stable]
- GIVEN a verified observation whose signed-refs values match before and after acquisition
- WHEN receipt admission runs
- THEN the observation MAY be accepted.

#### Scenario: Signed refs move during acquisition
r[kiln.radicle.source_contract.sigrefs.scenario.moved]
- GIVEN an observation whose before and after signed-refs commits differ
- WHEN receipt admission runs
- THEN the receipt MUST be rejected.

#### Scenario: Feature level is weaker than parent
r[kiln.radicle.source_contract.sigrefs.scenario.downgrade]
- GIVEN a valid signature at feature level `root` or `none`
- WHEN receipt admission runs
- THEN the receipt MUST be rejected.

### Requirement: Bounded canonical snapshot evidence
r[kiln.radicle.source_contract.snapshot] Snapshot evidence MUST bind the exact revision, native Git tree, canonical BLAKE3 identity, entry count, and total byte count under request bounds.

#### Scenario: Snapshot remains within bounds
r[kiln.radicle.source_contract.snapshot.scenario.accepted]
- GIVEN snapshot evidence that matches the request and remains within every bound
- WHEN receipt admission runs
- THEN the evidence MUST be accepted.

#### Scenario: Snapshot exceeds a bound
r[kiln.radicle.source_contract.snapshot.scenario.bound]
- GIVEN snapshot evidence whose entry or byte count exceeds the request
- WHEN receipt admission runs
- THEN the evidence MUST be rejected.

### Requirement: Exact receipt linkage
r[kiln.radicle.source_contract.linkage] A source receipt MUST reject signer, RID, reference, signed-refs commit, revision, or profile substitution.

#### Scenario: Receipt matches its request
r[kiln.radicle.source_contract.linkage.scenario.match]
- GIVEN a receipt whose observation and snapshot identities match the request
- WHEN receipt admission runs
- THEN the receipt MUST be accepted.

#### Scenario: Signer is substituted
r[kiln.radicle.source_contract.linkage.scenario.signer]
- GIVEN a receipt produced under another signer namespace
- WHEN receipt admission runs
- THEN the receipt MUST be rejected.

### Requirement: Source contract non-claims
r[kiln.radicle.source_contract.nonclaims] Contract admission MUST NOT claim repository trust, freshness, complete history, peer convergence, remote availability, build correctness, or release readiness.

#### Scenario: Contract admission succeeds
r[kiln.radicle.source_contract.nonclaims.scenario.accepted]
- GIVEN a structurally valid source receipt
- WHEN evidence is recorded
- THEN the evidence MUST remain limited to the supplied request, identities, bounds, and observations.
