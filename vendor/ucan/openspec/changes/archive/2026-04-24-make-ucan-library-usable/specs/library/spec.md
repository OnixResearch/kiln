## ADDED Requirements

### Requirement: End-to-end token round-trip

The `ucan` crate MUST let a consumer build, sign, serialize to a JWT/JWS-style compact token string carrying the first usable claim set (`iss`, `aud`, `att`, `prf`, `exp`, `nbf`), parse that string back, and verify a UCAN token through the public library API without hand-assembling token claims or orchestrating raw verified primitives directly.
r[library.token-round-trip]

#### Scenario: Consumer issues and verifies a token
r[library.token-round-trip.consumer-issues-and-verifies-a-token]

- GIVEN an Ed25519 issuer key, an audience DID, a non-empty capability set, and valid temporal bounds
- WHEN the consumer issues a compact token string, parses it back, and verifies it through the public `ucan` API
- THEN verification MUST succeed
- AND the verified result MUST expose the issuer, audience, capability set, proof list, and effective time window
- AND the flow MUST not require the consumer to call `sign_ed25519`, `verify_ed25519_signature`, or `validate_token_envelope` directly

#### Scenario: Invalid serialized token is rejected
r[library.token-round-trip.invalid-serialized-token]

- GIVEN a compact token string with malformed structure, invalid encoding, or missing required claims
- WHEN a consumer parses or verifies it through the public API
- THEN the library MUST return a structured error
- AND the library MUST not panic

### Requirement: did:key Ed25519 issuance and verification

The first usable library surface MUST support issuing and verifying tokens whose issuer identity is a `did:key` derived from an Ed25519 keypair.
r[library.did-key-ed25519]

#### Scenario: Issuer did:key matches the signing key
r[library.did-key-ed25519.issuer-did-key-matches-the-signing-key]

- GIVEN an Ed25519 secret key and a token issued through the public shell API
- WHEN the library derives the issuer `did:key`, signs the token, and later verifies the signature
- THEN the verified token MUST report the same issuer DID
- AND signature verification MUST succeed

#### Scenario: Signature mismatch is rejected
r[library.did-key-ed25519.signature-mismatch-is-rejected]

- GIVEN a token whose serialized content or signature bytes no longer match the issuer key
- WHEN the consumer verifies it through the public API
- THEN verification MUST fail with a signature-related error
- AND the library MUST not report the token as valid

### Requirement: Delegation proof chains are verified end to end

The library MUST verify delegation proof chains across parent tokens, including issuer/audience linkage, temporal narrowing, and capability attenuation.
r[library.delegation-chain]

#### Scenario: Narrower child token is accepted
r[library.delegation-chain.narrower-child-token-is-accepted]

- GIVEN a parent token and a child token whose proof links to the parent
- AND the child issuer matches the parent audience
- AND the child capability set is no broader than the parent capability set
- AND the child time window is no wider than the parent time window
- WHEN the library verifies the child token and its proof chain
- THEN verification MUST succeed
- AND the result MUST expose the effective delegation constraints

#### Scenario: Broken proof chain is rejected
r[library.delegation-chain.broken-proof-chain-is-rejected]

- GIVEN a token whose proof list omits a required parent, links to the wrong parent, or breaks the issuer/audience chain
- WHEN the library verifies the proof chain
- THEN verification MUST fail with a proof-chain error
- AND the token MUST not be treated as authorized

#### Scenario: Over-broad child delegation is rejected
r[library.delegation-chain.over-broad-child-delegation-is-rejected]

- GIVEN a child token whose capability set or expiration exceeds what its parent token delegates
- WHEN the library verifies the proof chain
- THEN verification MUST fail with an attenuation or temporal error
- AND the token MUST not be treated as authorized

#### Scenario: Self-delegation is rejected
r[library.delegation-chain.self-delegation-is-rejected]

- GIVEN a token whose issuer and audience are the same DID
- WHEN the `ucan` shell verifies the token or any proof chain containing it
- THEN verification MUST fail with a structured self-delegation error
- AND the token MUST not be treated as authorized

### Requirement: Functional core and imperative shell stay separate

Deterministic claim-shape parsing and structural validation MUST live in `crates/ucan-core`, while compact-token encoding/decoding, clocks, proof loading, and key resolution MUST live in the repo-root `ucan` crate.
r[library.fcis-boundary]

#### Scenario: Core validation remains pure
r[library.fcis-boundary.core-validation-remains-pure]

- GIVEN decoded token claims and capability values
- WHEN the library performs structural parsing and deterministic validation
- THEN that logic MUST be testable through `ucan-core` without filesystem, network, or clock dependencies

#### Scenario: Shell adapts validated values into verified semantics
r[library.fcis-boundary.shell-adapts-validated-values-into-verified-semantics]

- GIVEN a compact token string plus explicit time, proof, and key inputs
- WHEN the shell decodes it and verifies authorization semantics
- THEN it MUST bridge through `vendor/verified-logic` primitives instead of reimplementing attenuation, temporal, or signature rules locally

#### Scenario: Shell semantic bridge is auditable
r[library.fcis-boundary.shell-semantic-bridge-is-auditable]

- GIVEN the `ucan` shell crate source tree and the repo's verification checks
- WHEN the bridge audit runs
- THEN capability attenuation, token-time validation, and signature verification MUST remain delegated through `vendor/verified-logic`-backed functions
- AND the check MUST fail if equivalent local semantic implementations replace those bridges

### Requirement: `ucan-core` remains `no_std`

The token-claim model and structural validation work added to `crates/ucan-core` MUST preserve the crate's `#![no_std]` boundary.
r[library.core-no-std]

#### Scenario: Core compiles without `std`
r[library.core-no-std.core-compiles-without-std]

- GIVEN the `ucan-core` crate with the usable-library changes applied
- WHEN the repo runs the dedicated no-std verification rail for `ucan-core`
- THEN the crate MUST compile without `std`
- AND the new token-claim validation surface MUST remain available to that build

### Requirement: Consumer onboarding is part of the library surface

The repository MUST document the supported issuance and verification flow with runnable examples that match the public API.
r[library.consumer-onboarding]

#### Scenario: Fresh consumer follows the documented flow
r[library.consumer-onboarding.fresh-consumer-follows-the-documented-flow]

- GIVEN a fresh checkout and the documented example
- WHEN a consumer follows the README or example steps to issue and verify a token
- THEN the example MUST build against the public crate surface
- AND the flow MUST succeed without reading internal modules first

#### Scenario: Docs state current support limits
r[library.consumer-onboarding.docs-state-current-support-limits]

- GIVEN the initial usable-library release
- WHEN a consumer reads the documentation
- THEN it MUST clearly state the supported DID methods, signing algorithms, proof-loading model, and notable out-of-scope UCAN features
