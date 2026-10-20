## Why

Proof-chain verification still compared child issuer and parent audience directly in the shell. That duplicates one more UCAN semantic rule that the vendored `verified-logic` chain primitives already model.

## What Changes

- Add a small verified bridge helper for parent-audience/child-issuer proof links.
- Route proof-chain link checks through that bridge instead of direct shell string comparison.
- Strengthen bridge checks and tests so the local comparison does not drift back in.

## Impact

- **Files**: `src/verified.rs`, `src/token.rs`, `scripts/check-ucan-bridge.sh`, docs, library spec.
- **APIs**: Adds a public bridge helper; verification behavior remains compatible.
- **Testing**: Focused link tests plus the standard UCAN quality gates.
