## Why

Proof-chain traversal is one of the largest remaining ordinary Rust shell orchestration surfaces. The backend truth and token parsing remain runtime boundaries, but the traversal transcript, cycle rejection, budget admission, and accepted-chain delegation provenance are finite structural facts suitable for verified-logic.

## What Changes

- **1**: Add verified traversal transcript predicates for depth/count/input budgets and cycle/duplicate proof-reference rejection.
- **2**: Route accepted proof-chain traversal summaries through verified logic before effective delegation is reported.
- **3**: Keep proof-store truth, parser correctness, cryptography, and backend freshness as explicit boundaries.

## Capabilities

### New Capabilities
- `add-verified-proof-traversal-orchestration`: Specify verified structural proof traversal orchestration for budgets, cycles, and effective delegation transcripts.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
