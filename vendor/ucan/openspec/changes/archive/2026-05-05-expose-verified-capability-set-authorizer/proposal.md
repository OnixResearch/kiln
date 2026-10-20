## Why

The vendored `verified-logic` snapshot already includes a finite capability-set authorization primitive that checks whether any caller-supplied capability contains a concrete resource/ability request using byte-prefix resource refinement and exact ability equality. The UCAN shell exposes richer token authorization, but advanced callers do not yet have a direct public bridge for this exact finite set primitive.

## What Changes

- Add a public advanced helper that converts validated public capability documents into vendored `ucan_capability_merge::CapEntry` values.
- Delegate finite set authorization to `verified_logic::ucan_capability_merge::check_authorized_by`.
- Document that the helper is exact-resource-prefix/exact-ability data-only authorization, not token verification, wildcard subsumption, caveat policy, proof traversal, or production invocation verification.

## Impact

- Files: `src/verified.rs`, `src/lib.rs`, docs, bridge guard, OpenSpec library spec.
- APIs: new `capability_set_authorizes` verified bridge helper.
- Tests: focused positive/negative authorization tests plus normal UCAN gates.
