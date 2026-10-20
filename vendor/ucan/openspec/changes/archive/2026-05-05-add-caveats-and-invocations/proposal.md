## Why

The current model answers delegation questions: who delegated which resource/action to whom, for what time window. Reusable application crates also need request-specific authorization: an invoker asks to perform one operation, and caveats constrain the allowed request shape. Without a caveat and invocation model, every downstream crate must invent policy plumbing around the token library.

This is the fourth-priority follow-up because it unlocks modern UCAN-like application authorization after the safer signer, authorization decision, and resolver foundations exist.

## What Changes

- Add a caveat model to capabilities that can carry domain-specific constraints without putting policy I/O in `ucan-core`.
- Add policy hooks for caveat attenuation and request satisfaction.
- Add invocation request/result types that combine a token/proof chain, requested resource/ability, request facts, and caveat policy.
- Keep default behavior conservative: unknown caveats fail closed unless an explicit policy accepts them.

## Non-Goals

- Implement a universal caveat language.
- Add revocation registries or replay stores in this change.
- Chase UCAN spec interoperability; this targets reusable UCAN-like authorization inside this crate family.

## Capabilities

### New Capabilities
- `capability-caveats`: Capability claims can carry application constraints.
- `caveat-policy-hooks`: Callers can provide deterministic caveat attenuation and request-evaluation rules.
- `invocation-authorization`: Consumers can verify that a specific request is authorized by a token and proof chain.

### Modified Capabilities
- `authorization-decision-api`: Authorization decisions can include caveat-aware allow/deny reasons.
- `library.token-round-trip`: Token encoding/decoding preserves caveat payloads.

## Impact

- **Files**: `crates/ucan-core/src/token.rs`, `src/token.rs`, possible new `src/caveat.rs` / `src/invocation.rs`, examples, docs, tests.
- **APIs**: Adds caveat data types, policy traits, invocation request/result types, and caveat-aware verification paths.
- **Dependencies**: May use `serde_json` in the shell for compact-token caveat encoding; keep `ucan-core` codec-neutral.
- **Testing**: Needs positive and negative tests for caveat round-trip, attenuation, unknown-caveat failure, invocation allow/deny, and malformed caveat payloads.
