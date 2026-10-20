## Why

Verification currently proves that a token and its proof chain are valid, then exposes effective capabilities. A consumer still has to manually ask whether those effective capabilities authorize one concrete operation. Reusable crates should not duplicate this matching logic or convert structured verification errors into ad-hoc booleans.

This is the second-priority follow-up because application code usually needs an authorization decision, not just a verified token object.

## What Changes

- Add an authorization request type for `resource` plus `ability` checks against a verified token's effective delegation.
- Return structured allow/deny results that preserve the matched capability or denial reason.
- Reuse `VerifiedCapability` / verified-logic-backed containment semantics for resource and ability matching.
- Add examples and tests for allowed, over-broad, malformed, and no-match authorization checks.

## Non-Goals

- Add caveat evaluation in this change; leave extension points for the caveat/invocation change.
- Add revocation or external policy checks.
- Add network or storage dependencies.

## Capabilities

### New Capabilities
- `authorization-decision-api`: Consumers can ask whether a verified token authorizes a concrete resource/action.
- `structured-authorization-denials`: Denied checks return machine-readable reasons instead of only `false`.

### Modified Capabilities
- `library.delegation-chain`: Effective delegation becomes directly usable for request authorization.
- `library.consumer-onboarding`: Examples show the final authorize step after verification.

## Impact

- **Files**: `src/token.rs`, `src/verified.rs`, examples, README/API docs, tests.
- **APIs**: Adds authorization request/decision/result types and convenience methods on `VerifiedToken` or a verifier service.
- **Dependencies**: No new dependency expected.
- **Testing**: Needs positive and negative tests for exact matches, attenuated matches, no-match denials, and malformed request inputs.
