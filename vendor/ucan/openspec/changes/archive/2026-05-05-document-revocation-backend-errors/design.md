## Context

`RevocationChecker` already exposes backend errors through `RevocationError`, and verification maps them to `TokenError::Revocation`. Unit tests cover fail-closed behavior; this change adds consumer-facing executable evidence.

## Decisions

### Demonstrate a local failing checker

**Choice:** Add an example-local checker that returns `RevocationError::Backend` for the reference it is asked to check.

**Rationale:** This is deterministic, has no network/storage dependency, and demonstrates the production contract without adding a fake registry implementation.

### Print structured class, not token material

**Choice:** Print the backend reference and message from the structured error, but never print compact token strings.

**Rationale:** The example should be useful for application error mapping while preserving bearer-token hygiene.

## Non-Goals

- No retry policy, circuit breaker, registry schema, credential-status format, or persistence abstraction.
- No change to core no-std behavior.
