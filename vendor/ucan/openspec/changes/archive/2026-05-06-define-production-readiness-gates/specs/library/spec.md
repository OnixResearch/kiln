## ADDED Requirements

### Requirement: Production-readiness support labels [r[library.production-readiness.support-labels]]

The library MUST publish evidence-backed support labels for every documented production-relevant surface, distinguishing controlled-integration support from preview and production-supported platform behavior.

#### Scenario: Current bounded library surface is labeled explicitly [r[library.production-readiness.support-labels.current-controlled-integration]]

- GIVEN the current library supports Ed25519 `did:key` issuance and verification with explicit proof, revocation, caveat-policy, verification-time, and limit inputs
- WHEN readiness documentation or machine-readable reports describe that surface
- THEN they MUST label it as controlled-integration ready or stronger only when the aggregate verification gate proves the documented checks
- AND they MUST NOT imply built-in remote resolver, proof transport, revocation registry, replay store, persistence, non-Ed25519, or arbitrary caveat support

#### Scenario: Production-supported labels require matching evidence [r[library.production-readiness.support-labels.production-supported-evidence]]

- GIVEN a surface is labeled `production-supported`
- WHEN the readiness check evaluates the repository
- THEN it MUST find positive tests, negative/fail-closed tests, docs, and operator-facing evidence for that exact surface
- AND the check MUST fail if README, API docs, boundary docs, or readiness reports disagree about the support level

### Requirement: Explicit production integration profile [r[library.production-readiness.integration-profile]]

The library MUST define a production integration profile that composes caller-owned infrastructure through explicit public APIs rather than hidden global network, storage, clock, registry, or key-custody behavior.

#### Scenario: Integration profile wires all production-owned inputs [r[library.production-readiness.integration-profile.explicit-wiring]]

- GIVEN an application wants to use UCAN verification in production
- WHEN it follows the production integration profile
- THEN the profile MUST show explicit `KeyResolver`, `ProofStore`, `RevocationChecker`, verification time, verification limits, caveat policies, and signer/key-custody boundaries
- AND the profile MUST document which backend truth, freshness, persistence, and provenance claims remain application-owned

#### Scenario: Integration profile fails closed on backend and policy failures [r[library.production-readiness.integration-profile.fail-closed]]

- GIVEN resolver, proof-store, revocation, replay, or caveat-policy infrastructure reports an error, stale value, denial, duplicate, or missing policy
- WHEN the integration profile verifies or authorizes a token or invocation
- THEN verification or authorization MUST fail closed with structured errors or denials
- AND the failure evidence MUST avoid logging compact token secrets or signing material

### Requirement: Remote DID resolution promotion gate [r[library.production-readiness.remote-did-gate]]

Remote DID resolution MUST remain below `production-supported` until resolver trust, freshness, method support, cache behavior, and fail-closed errors are documented and tested.

#### Scenario: Unsupported or stale resolver output blocks promotion [r[library.production-readiness.remote-did-gate.unsupported-or-stale]]

- GIVEN a DID method or resolver response is unsupported, stale, malformed, mismatched, or unavailable
- WHEN production-readiness evidence evaluates remote DID resolution
- THEN the gate MUST require verification to fail closed before key material is trusted
- AND the readiness report MUST keep remote DID resolution out of production-supported status until that evidence exists

### Requirement: Proof transport promotion gate [r[library.production-readiness.proof-transport-gate]]

Network, filesystem, database, or content-addressed proof transport MUST remain below `production-supported` until proof provenance, freshness, bounds, reference matching, and failure behavior are documented and tested.

#### Scenario: Bad proof material blocks promotion [r[library.production-readiness.proof-transport-gate.bad-proof-material]]

- GIVEN proof material is missing, oversized, stale, cyclic, malformed, or does not match its proof reference
- WHEN production-readiness evidence evaluates proof transport
- THEN the gate MUST require verification to fail closed without treating the proof as a valid delegation parent
- AND successful proof transport MUST still route chain semantics through the existing verified proof-chain bridges

### Requirement: Registry-backed revocation promotion gate [r[library.production-readiness.revocation-registry-gate]]

Registry-backed revocation MUST remain below `production-supported` until registry truth, freshness, duplicate handling, backend errors, and leaf/proof denial behavior are documented and tested.

#### Scenario: Registry denial and backend failures fail closed [r[library.production-readiness.revocation-registry-gate.fail-closed]]

- GIVEN a revocation registry denies the leaf token, denies a proof-chain token, returns stale evidence, reports duplicate conflicting entries, or fails
- WHEN production-readiness evidence evaluates registry-backed revocation
- THEN the gate MUST require verification to reject the token or fail closed with a structured revocation error
- AND the readiness report MUST distinguish deterministic in-memory revocation support from registry-backed production support

### Requirement: Replay and persistence promotion gate [r[library.production-readiness.replay-persistence-gate]]

Replay protection and persistent receipt/state storage MUST remain below `production-supported` until duplicate detection, restart recovery, corruption handling, and freshness boundaries are documented and tested.

#### Scenario: Replayed invocation is rejected after recovery [r[library.production-readiness.replay-persistence-gate.replay-after-recovery]]

- GIVEN an invocation or receipt has already been accepted before process restart
- WHEN the same invocation or receipt is presented after recovery from persistent state
- THEN the gate MUST require the replay to be rejected deterministically
- AND storage corruption, missing state, or freshness uncertainty MUST fail closed rather than silently authorizing mutation

### Requirement: Expanded key algorithm promotion gate [r[library.production-readiness.key-algorithm-gate]]

Any signing algorithm or DID/key type beyond Ed25519 `did:key` MUST remain below `production-supported` until parsing, admission, signing, verification, mismatch, and unsupported-algorithm behavior are documented and tested.

#### Scenario: Non-Ed25519 support requires explicit algorithm evidence [r[library.production-readiness.key-algorithm-gate.explicit-evidence]]

- GIVEN a token, signer, or resolver presents a non-Ed25519 key type or non-`EdDSA` algorithm
- WHEN production-readiness evidence evaluates expanded key support
- THEN the gate MUST require explicit positive tests for supported algorithms and negative tests for header/signature/key mismatches
- AND unsupported algorithms MUST fail closed without being reported as production-supported

### Requirement: Broader caveat vocabulary promotion gate [r[library.production-readiness.caveat-vocabulary-gate]]

Caveat semantics beyond the currently supported typed subset and caller-supplied opaque policy evaluation MUST remain below `production-supported` until payload parsing, policy truth, clock/counter provenance, and failure behavior are documented and tested.

#### Scenario: Unknown or malformed caveats block authorization [r[library.production-readiness.caveat-vocabulary-gate.unknown-or-malformed]]

- GIVEN a caveat is unknown, malformed, has untrusted clock/counter provenance, or is rejected by its policy
- WHEN production-readiness evidence evaluates broader caveat vocabulary support
- THEN authorization MUST fail closed
- AND the readiness report MUST distinguish verified finite typed caveat helpers from application-owned arbitrary caveat semantics

### Requirement: Aggregate production-readiness gate [r[library.production-readiness.aggregate-gate]]

The repository MUST provide an aggregate readiness gate that checks docs, examples, tests, and machine-readable support status before any surface is promoted to production-supported.

#### Scenario: Readiness gate prevents overclaiming [r[library.production-readiness.aggregate-gate.prevents-overclaiming]]

- GIVEN README or public API docs claim a production-supported surface
- WHEN the aggregate readiness gate runs
- THEN it MUST verify that surface has matching implementation evidence, negative tests, boundary docs, and support-status entries
- AND it MUST fail if the claim exceeds the evidence captured in the repository
