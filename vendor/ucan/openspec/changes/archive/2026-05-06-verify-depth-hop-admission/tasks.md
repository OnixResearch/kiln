## Phase 1: Specification

- [x] Create a focused OpenSpec package for proof-hop depth admission.

## Phase 2: Sibling verified-logic primitive

- [x] Add an executable `ucan_delegation_depth` primitive for proof-hop depth admission and overflow rejection.
- [x] Add sibling positive/negative tests and run focused Verus/Cargo verification.
- [x] Commit and push the sibling primitive.

## Phase 3: UCAN integration

- [x] Sync vendored `verified-logic` into UCAN and redact source repo metadata in summaries.
- [x] Route `proof_depth_within_budget` through the vendored proof-hop helper without changing public API shape.
- [x] Update UCAN tests, docs, bridge coverage, and guard checks.

## Phase 4: Verification and archive

- [x] Run UCAN validation: fmt, workspace tests, clippy, no-std guard, bridge guard, OpenSpec validation, `git diff --check`, and `nix flake check /home/brittonr/git/ucan`.
- [x] Archive the OpenSpec, commit, push, and verify clean local/tracking/live remote parity.
