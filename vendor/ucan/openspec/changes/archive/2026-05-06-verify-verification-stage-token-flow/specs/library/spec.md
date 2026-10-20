## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST verify live token-verification stage-flow transcripts so emitted stage facts appear in the documented order and the first failed stage determines the structured verification failure before authorization returns.

#### Scenario: Live verification flow uses verified stage bridge [r[library.compact-token.scenario.verification-stage-token-flow-admission]]

- GIVEN a compact-token verification attempt that emits parse, structural, key, signature, proof/backend, revocation, and authorization stage facts
- WHEN the shell decides the final verification result
- THEN it MUST route ordered stage facts and first-failure facts through a vendored verified stage-flow bridge
- AND a failed earlier stage MUST prevent later success from authorizing the token

#### Scenario: Stage-flow admission keeps stage truth outside the verified claim [r[library.compact-token.scenario.verification-stage-token-flow-boundary-limits]]

- GIVEN verification stage-flow facts admitted by the verified predicate
- WHEN the shell relies on parser, crypto, backend, revocation, or authorization checks
- THEN the library MUST continue to document the truth of parser outputs, cryptographic verification, backend answers, revocation freshness, proof material provenance, caveat policies, and token provenance as outside the stage-flow admission claim
