## Why

Opaque caveat preservation is bridged by identifier bytes, but parsed caveat-list shape and identifier-input preservation are still shell-local. The closeable fact is finite: each parsed caveat entry exposes the identifier inputs and payload-presence bytes used to build caveat identifiers.

## What Changes

- Add a vendored verified primitive for parsed caveat-list entry shape and identifier-input preservation facts.
- Route parsed capability caveat lists through a UCAN bridge before caveat identifier construction/preservation checks.
- Keep payload parser correctness, application semantics, clock/counter provenance, and arbitrary caveat policies outside the verified claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, caveat parsing/preservation path, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a parsed caveat-list shape/preservation bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
