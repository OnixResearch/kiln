## Why

The UCAN shell intentionally treats caveat documents as opaque application-defined payloads, but vendored `verified-logic` contains finite typed caveat predicates for contexts such as time bounds and count/rate windows. Callers that choose those typed semantics should be able to use them without replacing the shell's fail-closed caller-supplied caveat policy boundary.

## What Changes

- Add optional public verified helpers for the vendored typed caveat predicates.
- Keep shell `CaveatPolicy` and unknown-caveat fail-closed behavior unchanged.
- Document that typed helpers are opt-in building blocks for policy implementations, not automatic caveat semantics for all UCANs.

## Capabilities

### New Capabilities
- `library.verified-typed-caveat-helpers`: Application policy authors can preflight supported typed caveats through vendored verified-logic.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, README/API docs, bridge guard script, OpenSpec library spec.
- **APIs**: New public typed caveat helper functions and small public input wrappers if needed.
- **Dependencies**: No new runtime dependencies.
- **Testing**: Focused allowed/denied typed caveat cases, unknown caveat regression, bridge guard, full validation.
