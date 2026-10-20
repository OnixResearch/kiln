## Phase 1: Specification

- [x] Create a focused OpenSpec package for verified ability concrete/wildcard admission.

## Phase 2: Sibling verified-logic primitive

- [x] Add an executable `ucan_ability` primitive for wildcard-segment presence / concrete admission.
- [x] Add sibling positive/negative tests and run focused Verus/Cargo verification.
- [x] Commit and push the sibling primitive.

## Phase 3: UCAN integration

- [x] Sync vendored `verified-logic` into UCAN and redact source repo metadata.
- [x] Route `VerifiedCapability::{has_wildcard_segment,is_concrete}` through the vendored primitive without changing public API shape.
- [x] Update UCAN tests, docs, bridge coverage, and guard checks.

## Phase 4: Verification and archive

- [x] Run UCAN validation: fmt, workspace tests, clippy, no-std guard, bridge guard, OpenSpec validation, `git diff --check`, and `nix flake check /home/brittonr/git/ucan`.
- [x] Archive the OpenSpec, commit, push, and verify clean local/tracking/live remote parity.
