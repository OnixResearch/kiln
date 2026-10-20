## Context

`RevocationList` is an in-memory helper implementation of the caller-injected `RevocationChecker` trait. The verification path uses `revocation_reference_is_listed`, but `revoke` still performed duplicate detection with `self.0.iter().any(...)`.

## Decisions

### Use the existing verified revocation bridge for helper dedupe

`revoke` will build the same byte-vector view used by `is_revoked` and call `revocation_reference_is_listed(existing, reference)`. This keeps helper insertion idempotence tied to the same finite-set semantics as verification denial.

### Keep fail-closed verification unchanged

The bridge helper currently returns `bool`, so no backend-error or verification-error behavior changes. `RevocationChecker` implementations remain caller-injected and shell-only.

## Non-Goals

- No network/database/registry revocation source.
- No replay store or revocation freshness semantics.
- No changes to compact token proof reference derivation.
