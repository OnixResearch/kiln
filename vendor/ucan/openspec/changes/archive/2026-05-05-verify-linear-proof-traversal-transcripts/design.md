## Context

`verified_logic::ucan_proof_traversal` validates finite linear transcripts ordered from parent to child. The UCAN shell verifier currently verifies recursively with caller-injected resolvers/stores and computes effective delegation in ordinary Rust.

## Decisions

### Linear transcript only

The integration SHALL emit transcripts only for root/no-proof and exactly-one-parent proof chains whose represented edges use resource-prefix attenuation with exact ability equality. Multi-parent proof sets and wildcard-ability attenuation remain accepted or denied by existing shell checks and are explicitly not marked as verified by this primitive.

### Fail closed for eligible transcripts

For eligible chains, the shell SHALL build entries from already parsed, signature-verified, revocation-checked token data and SHALL return a proof-chain error if the verified bridge rejects the transcript.

### Boundary preservation

The transcript builder SHALL not resolve proofs, query revocations, verify signatures, parse compact tokens, read clocks, or evaluate caveat policies. It only adapts already-available shell data into bridge input.

## Risks

- The primitive's caveat model is ordered and linear; the shell must construct a deterministic effective caveat-id prefix from the validated parent transcript rather than pretending to prove arbitrary raw caveat ordering.
- Multi-parent chains need a future finite DAG primitive before equivalent bridge enforcement.
