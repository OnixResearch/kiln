## Phase 1: Spec Foundation

- [x] [serial] Create proposal, design, task plan, and delta spec for generic consumer integration hooks.
- [x] [serial] Capture proposal/design/tasks gate evidence or document unavailable gate tooling before implementation.

## Phase 2: Public Hook API

- [x] [serial] Add public generic hook traits for DID/key resolution, proof loading, revocation checking, and replay/nonce admission without Aspen or Clankers-specific vocabulary.
- [x] [depends:hook-api] Add structured error taxonomy and safe display/debug behavior for hook failures.
- [x] [depends:hook-api] Wire hook-backed verification/integration profile APIs through existing token/proof/revocation semantics without breaking existing public APIs.

## Phase 3: Conformance and Readiness Evidence

- [x] [depends:hook-api] Add deterministic fake/in-memory hook implementations for tests and examples only.
- [x] [depends:hook-api] Add positive conformance tests proving hook-backed resolution, proof loading, revocation, and replay admission can authorize valid inputs.
- [x] [depends:hook-api] Add negative conformance tests proving missing, stale, malformed, denied, duplicate, and backend-error cases fail closed.
- [x] [depends:hook-api] Update readiness docs and machine-readable report so hook traits are distinguished from concrete production backend support.
- [x] [depends:readiness] Extend `scripts/check-production-readiness.py` to reject overclaiming of concrete backend support when only hook traits exist.

## Phase 4: Documentation, Verification, and Archive

- [x] [depends:hook-api] Document the consumer responsibility split for Aspen, Clankers, and other integrators without adding consumer-specific adapters.
- [x] [depends:tests] Run targeted tests, full library tests, clippy, readiness check, strict OpenSpec validation, and `nix flake check` with evidence captured under this change.
- [x] [depends:verification] Archive the OpenSpec change after all implementation and evidence tasks are complete, then validate all specs and commit/push the archive.
