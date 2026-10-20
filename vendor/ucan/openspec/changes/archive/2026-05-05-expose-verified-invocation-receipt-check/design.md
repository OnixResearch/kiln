## Context

The vendored `verified_logic::ucan_invocation` module exposes `Receipt`, `Invocation`, `InvocationResult`, and `check_receipt_valid`. Its scope is structural: the executor must match the invocation invoker and the receipt timestamp must be at or after the invocation timestamp.

## Decisions

### Public shell bridge

Expose a string/timestamp helper instead of a full receipt model. The shell converts public DID strings and deterministic timestamps into the vendored receipt type and returns the verified checker result.

### Non-goals

No receipt persistence, replay store, receipt signing, output/error serialization format, network lookup, default registry, or ambient clock reads. The helper is deterministic and caller-input only.

## Validation

Run focused bridge tests, the bridge guard, strict OpenSpec validation, full Cargo tests/clippy/fmt, no-std guard, diff check, and Nix flake check.
