## ADDED Requirements

### Requirement: Verified verification-stage transcript boundary [r[library.verified-verification-stage-transcript-boundary]]

The library MUST expose a data-only verified transcript boundary for finite UCAN verification stage facts, and MUST accept a transcript only when every required stage fact is successful. The bridge MUST NOT claim that parser internals, base64url, JSON serde/canonicalization, Ed25519, BLAKE3, resolver/proof-store/revocation backend correctness, proof freshness, filesystem/network behavior, caveat policies, signing, persistence, replay protection, or ambient clocks are formally verified.

#### Scenario: Accepted transcript requires every stage [r[library.verified-verification-stage-transcript-boundary.scenario.accepts-all-stages]]

- GIVEN a transcript whose parse, structural, key-resolution, signature, envelope, revocation, proof-load, proof-chain, and authorization facts all pass
- WHEN the verified stage transcript bridge evaluates it
- THEN the bridge MUST accept it
- AND the first-failure stage MUST be the accepted sentinel

#### Scenario: Rejected transcript reports first failed stage [r[library.verified-verification-stage-transcript-boundary.scenario.rejects-first-failure]]

- GIVEN a transcript with one or more failed stage facts
- WHEN the verified stage transcript bridge evaluates it
- THEN the bridge MUST reject it
- AND the first-failure stage MUST identify the earliest failed stage in fixed shell order without exposing token bytes or backend payloads

#### Scenario: Runtime trust boundaries remain explicit [r[library.verified-verification-stage-transcript-boundary.scenario.trust-boundaries]]

- GIVEN shell verification uses runtime codec, cryptographic, resolver, proof-store, revocation, and caveat-policy code
- WHEN documentation or guard scripts describe stage transcript coverage
- THEN they MUST state that the transcript bridge consumes caller-supplied facts only
- AND they MUST NOT describe backend correctness, cryptographic validity, parsing semantics, proof freshness, or caveat policy satisfaction as verified by this bridge
