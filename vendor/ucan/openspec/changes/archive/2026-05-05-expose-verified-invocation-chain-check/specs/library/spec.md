## ADDED Requirements

### Requirement: Verified invocation-chain authorization bridge [r[library.verified-bridges.invocation-chain]]

The library MUST expose a deterministic public helper that delegates caller-supplied resolved invocation-chain authorization data to the vendored verified invocation-chain composition primitive.

#### Scenario: Resolved chain authorizes concrete invocation [r[library.verified-bridges.invocation-chain.authorized]]

- GIVEN caller-supplied resolved chain entries, aligned proof-reference bytes, no matching revoked references, an invocation authorization reference equal to the leaf proof reference, a valid leaf capability index, and deterministic time inputs
- WHEN the helper is called with an invoker matching the leaf audience and a request whose resource refines the leaf resource with the same ability
- THEN the helper MUST return true through the verified invocation-chain checker

#### Scenario: Revoked or unbound invocation denies [r[library.verified-bridges.invocation-chain.denied]]

- GIVEN caller-supplied resolved chain entries and aligned proof-reference bytes
- WHEN any aligned proof reference is listed as revoked, the chain is structurally invalid, the invocation authorization reference differs from the leaf proof reference, the leaf capability index is out of bounds, or the request escalates resource or ability
- THEN the helper MUST return false through the verified invocation-chain checker

#### Scenario: Advanced bridge preserves shell-owned behavior [r[library.verified-bridges.invocation-chain.boundary]]

- GIVEN production verification requires proof resolution, cryptographic signature verification, fail-closed revocation backends, wildcard containment, and caveat policy evaluation
- WHEN the invocation-chain bridge is used
- THEN the helper MUST remain limited to caller-supplied finite data and MUST NOT perform proof fetching, compact-token verification, cryptography, DID/key lookup, revocation backend or freshness checks, caveat policy evaluation, wildcard containment replacement, registry/storage/network access, persistence, replay checks, or ambient clock reads
