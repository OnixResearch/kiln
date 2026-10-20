## MODIFIED Requirements

### Requirement: Explicit production integration profile [r[library.production-readiness.integration-profile]]

The library MUST define a production integration profile that composes caller-owned infrastructure through explicit public APIs rather than hidden global network, storage, clock, registry, or key-custody behavior, and the profile MUST include generic hook traits that can be implemented by multiple consumers without embedding consumer-specific policy in `ucan`.

#### Scenario: Integration profile exposes generic hook seams [r[library.production-readiness.integration-profile.generic-hook-seams]]

- GIVEN applications such as Aspen and Clankers need to consume `ucan` with different identity, authorization, persistence, and RPC/session policies
- WHEN the library documents and exposes its production integration profile
- THEN it MUST provide generic UCAN-native hook seams for DID/key resolution, proof loading, revocation checking, and replay or nonce admission
- AND those seams MUST NOT contain Aspen capability names, Clankers tool/session names, or any other consumer-specific authorization vocabulary

#### Scenario: Hook failures fail closed with structured errors [r[library.production-readiness.integration-profile.hook-fail-closed]]

- GIVEN any resolver, proof loader, revocation checker, replay admission hook, or caveat-policy hook reports missing, stale, malformed, denied, duplicate, unavailable, or backend-error evidence
- WHEN hook-backed verification or authorization evaluates a token, proof chain, or invocation
- THEN the library MUST reject the token or invocation with a structured fail-closed error or denial
- AND safe error output MUST avoid compact token contents, private keys, bearer credentials, or signing material

### Requirement: Remote DID resolution promotion gate [r[library.production-readiness.remote-did-gate]]

Remote DID resolution MUST remain below `production-supported` until resolver trust, freshness, method support, cache behavior, and fail-closed errors are documented and tested, and the library MUST distinguish a generic resolver hook from any concrete remote resolver backend.

#### Scenario: Generic resolver hook does not imply backend support [r[library.production-readiness.remote-did-gate.generic-hook-not-backend]]

- GIVEN `ucan` exposes a generic DID or key resolver hook
- WHEN readiness documentation or machine-readable reports describe resolver support
- THEN they MUST label the hook contract separately from concrete remote DID method or network backend support
- AND they MUST NOT mark remote DID resolution as `production-supported` without backend-specific trust, freshness, cache, negative-test, and operator-evidence artifacts

### Requirement: Proof transport promotion gate [r[library.production-readiness.proof-transport-gate]]

Network, filesystem, database, or content-addressed proof transport MUST remain below `production-supported` until proof provenance, freshness, bounds, reference matching, and failure behavior are documented and tested, and the library MUST distinguish a generic proof-loader hook from any concrete proof transport backend.

#### Scenario: Generic proof loader hook does not imply transport support [r[library.production-readiness.proof-transport-gate.generic-hook-not-backend]]

- GIVEN `ucan` exposes a generic proof-chain loading hook
- WHEN readiness documentation or machine-readable reports describe proof support
- THEN they MUST label the hook contract separately from concrete network, filesystem, database, or content-addressed proof transport support
- AND bad proof references, missing proofs, stale proofs, cycles, mismatches, and backend errors MUST fail closed in conformance tests before any concrete proof transport can be promoted

### Requirement: Registry-backed revocation promotion gate [r[library.production-readiness.revocation-registry-gate]]

Registry-backed revocation MUST remain below `production-supported` until registry truth, freshness, duplicate handling, backend errors, and leaf/proof denial behavior are documented and tested, and the library MUST distinguish a generic revocation checker hook from any concrete registry backend.

#### Scenario: Generic revocation checker hook does not imply registry support [r[library.production-readiness.revocation-registry-gate.generic-hook-not-backend]]

- GIVEN `ucan` exposes a generic revocation checker hook
- WHEN readiness documentation or machine-readable reports describe revocation support
- THEN they MUST label the hook contract separately from concrete registry-backed revocation support
- AND revoked leaves, revoked proofs, stale registry evidence, duplicate conflicting entries, and backend failures MUST fail closed in conformance tests before any concrete registry can be promoted

### Requirement: Replay and persistence promotion gate [r[library.production-readiness.replay-persistence-gate]]

Replay protection and persistent receipt/state storage MUST remain below `production-supported` until duplicate detection, restart recovery, corruption handling, and freshness boundaries are documented and tested, and the library MUST distinguish a generic replay admission hook from any concrete persistence backend.

#### Scenario: Generic replay admission hook does not imply persistence support [r[library.production-readiness.replay-persistence-gate.generic-hook-not-backend]]

- GIVEN `ucan` exposes a generic replay or nonce admission hook
- WHEN readiness documentation or machine-readable reports describe replay support
- THEN they MUST label the hook contract separately from concrete persistence, recovery, or receipt-store support
- AND duplicate admission, stale admission state, corrupted state, missing state, and backend failures MUST fail closed in conformance tests before any concrete replay store can be promoted

### Requirement: Aggregate production-readiness gate [r[library.production-readiness.aggregate-gate]]

The repository MUST provide an aggregate readiness gate that checks docs, examples, tests, and machine-readable support status before any surface is promoted to production-supported, including a check that generic hook traits are not overclaimed as concrete production backends.

#### Scenario: Hook traits do not overpromote concrete backend readiness [r[library.production-readiness.aggregate-gate.hooks-not-backends]]

- GIVEN a README, API doc, readiness report, or example claims resolver, proof transport, revocation, or replay support
- WHEN the aggregate readiness gate evaluates that claim
- THEN it MUST distinguish generic hook contract support from concrete backend support
- AND it MUST fail if a concrete backend is labeled `production-supported` based only on trait definitions, fake implementations, or consumer-owned infrastructure outside this repository
