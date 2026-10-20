## Why

The shell invocation workflow verifies tokens, authorizes requested operations, and evaluates caveats, but the concrete invocation admission relation itself is not explicitly bridged to the vendored `verified_logic::ucan_invocation` primitive when a matched capability is concrete.

## What Changes

- Add a shell bridge helper for the verified invocation authorization primitive.
- Route concrete matched-capability invocation admissions through that helper after existing caveat-aware authorization succeeds.
- Preserve existing wildcard-delegation and caveat semantics by using the helper only when the matched capability is concrete.
- Clarify that invocation exposure uses the token audience as the invoker exercising the delegated capability.

## Impact

- Files: `src/verified.rs`, `src/token.rs`, `src/lib.rs`, docs, bridge guard, library spec.
- APIs: additive public bridge helper; invocation result semantics become aligned with delegated audience/invoker semantics.
- Verification: focused invocation tests, bridge guard, full Cargo/Nix/OpenSpec gates.
