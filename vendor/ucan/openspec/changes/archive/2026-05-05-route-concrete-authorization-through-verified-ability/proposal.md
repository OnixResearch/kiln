## Why

Concrete authorization requests already reject wildcard ability segments, but that check is implemented with a local string split in the `ucan` shell. Ability parsing and wildcard semantics are a verified-logic-backed boundary elsewhere, so request concreteness should route through the same bridge instead of carrying a parallel shell-side semantic helper.

## What Changes

- Add a small verified-logic-backed capability concreteness helper.
- Use it from `AuthorizationRequest` admission when rejecting wildcard request abilities.
- Guard the bridge so future local wildcard admission checks do not bypass verified ability parsing.

## Capabilities

### Modified Capabilities
- `library.authorization-decision-api`: concrete request admission remains unchanged externally, but wildcard rejection is delegated through the verified ability bridge.

## Impact

- **Files**: `src/verified.rs`, `src/token.rs`, bridge guard script, docs/tests.
- **APIs**: additive `VerifiedCapability` helper; no breaking API change.
- **Dependencies**: none.
- **Testing**: focused tests, fmt, clippy, bridge/no-std guards, OpenSpec validation, Nix.
