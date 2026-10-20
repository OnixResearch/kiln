## Phase 1: Spec Foundation

- [x] [serial] Write proposal, design, delta spec, and implementation task plan for `add-verified-ability-helper-predicates` ✅

## Phase 2: Verified Primitive Implementation

- [x] [serial] Add the focused verified-logic primitive(s), proof contracts, and exported helpers for this seam.
- [x] [depends:verified-primitives] Sync the vendored `verified-logic` snapshot into `ucan` with `./scripts/sync-verified-logic.sh` and record the imported revision.

## Phase 3: Shell Integration

- [x] [depends:vendored-snapshot] Route the `ucan` shell/core path through the new verified bridge without adding consumer-specific adapters.
- [x] [parallel] Update README, public API, verified-boundary, and production-readiness docs to describe the new verified claim and retained boundaries.
- [x] [parallel] Add positive, negative, and boundary tests/examples for the routed behavior.

## Phase 4: Verification and Archive

- [x] [depends:shell-integration] Run focused cargo tests, readiness/bridge checks, OpenSpec validation, whitespace checks, and full `nix flake check`.
- [x] [depends:verification] Capture implementation verification evidence under this change.
- [x] [depends:evidence] Archive the OpenSpec after all implementation and verification tasks are complete.
