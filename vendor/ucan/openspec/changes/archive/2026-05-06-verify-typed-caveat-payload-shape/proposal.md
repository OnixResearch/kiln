## Why

The typed caveat adapter currently parses the supported time-bound and rate-limit payload bytes in ordinary UCAN shell code before constructing vendored typed caveats. The shape checks are deterministic and finite: non-empty ASCII decimal payloads for time bounds, and exactly one colon separating two non-empty ASCII decimal payloads for rate limits. Routing those payload-shape admission checks through `verified-logic` removes another shell-local fact without changing caller-owned caveat policy semantics.

## What Changes

- Add executable payload-shape helpers to `verified_logic::ucan_caveat` for supported typed caveat payload formats.
- Route UCAN `parse_time_bound_caveat` and `parse_rate_limit_caveat` through the vendored helpers before runtime UTF-8/u64 parsing.
- Update boundary docs and bridge guards so the payload-shape checks remain routed.

## Impact

- **Files**: `../verified-logic/src/ucan_caveat.rs`, vendored snapshot, UCAN typed caveat parser, docs, bridge guard.
- **APIs**: no caller-facing API change; helper internals fail closed on malformed typed caveat payloads.
- **Verification**: sibling Verus/tests plus UCAN fmt/tests/clippy/no-std/bridge/OpenSpec/flake gates.
