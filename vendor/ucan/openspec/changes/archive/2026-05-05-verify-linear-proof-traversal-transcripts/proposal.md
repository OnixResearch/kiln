## Why

UCAN now exposes verified finite proof-traversal transcript primitives, but the live verifier still performs proof traversal without emitting a checked transcript. A focused shell integration can add fail-closed verification for the linear proof-chain shape that the primitive exactly models, without moving resolver, revocation, parsing, signature, caveat-policy, clock, or backend behavior into verified-logic.

## What Changes

- Emit an internal parent-to-child transcript for no-proof verification and single-parent proof-chain verification whose edges match the current primitive's exact resource-prefix and exact-ability model.
- Check eligible transcripts with `proof_traversal_transcript_is_valid` before returning a verified token.
- Preserve existing synchronous caller-injected key resolver, proof store, revocation checker, compact codec, and caveat-policy boundaries.
- Keep multi-parent proof DAGs and wildcard-ability attenuation on the existing shell path until verified primitives exist for those exact models.

## Out of Scope

- Fetching proofs, parsing compact tokens, signature verification, revocation backend correctness, replay storage, ambient clocks, persistence, networking, or arbitrary caveat semantics inside verified-logic.
- Claiming multi-parent proof DAGs are covered by the current linear transcript primitive.

## Verification

Run focused proof-chain tests, bridge guard, OpenSpec strict validation, and the repo's normal Cargo/Nix gates.
