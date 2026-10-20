## Context

`ucan` is controlled-integration-ready and composes a tested Rust shell with vendored finite verified-logic predicates. The next verification improvements should move narrow structural facts into `verified-logic` without claiming parser, crypto, backend, storage, or consumer-policy truth.

## Goals / Non-Goals

**Goals:**
- Verify finite replay-admission outcome aggregation and first-failure/fail-closed semantics.
- Preserve caller-owned replay storage and backend behavior boundaries.

**Non-Goals:**
- No persistent replay store.
- No proof of backend freshness or crash consistency.
- No Aspen/Clankers request-envelope semantics.

## Decisions

### 1. Verify finite transcripts and admission facts only

**Choice:** This change targets finite predicates over shell-supplied facts and deterministic structural inputs.

**Rationale:** These predicates are small enough to prove and useful enough to reduce shell trust. They do not require proving external cryptography, codecs, clocks, storage, networking, or consumer policy.

**Alternative:** Prove the entire end-to-end UCAN verifier. Rejected because the root crate remains an ordinary Rust shell around parser, codec, crypto, and caller-owned backend boundaries.

**Implementation:** Add or extend vendored `verified-logic` primitives, sync the vendored snapshot, route the shell through the new bridges, and add positive/negative tests plus boundary documentation.

### 2. Preserve consumer-owned integration semantics

**Choice:** Aspen, Clankers, and other consumers continue to implement their own adapters and policy mappings around generic `ucan` hooks.

**Rationale:** `ucan` must remain generic and reusable across multiple downstream consumers.

**Alternative:** Add Aspen- or Clankers-specific adapter behavior inside `ucan`. Rejected by project boundary and user direction.

## Risks / Trade-offs

**Overclaiming formal verification** → Mitigate by documenting every runtime/backend/crypto/parser boundary and keeping readiness at `controlled-integration-ready` until concrete production backends exist.

**Vendored snapshot drift** → Mitigate by using `./scripts/sync-verified-logic.sh`, recording the imported revision, and running full Nix checks after vendoring.

**Archive clobbering canonical scenarios** → Mitigate by inspecting canonical `openspec/specs/library/spec.md` diffs after archive before commit.

## Validation Plan

- `openspec validate add-verified-replay-admission-transcript --strict --json`
- `python ~/.hermes/skills/agentkit-port/openspec/scripts/openspec_helper.py verify add-verified-replay-admission-transcript --json` with incomplete-task warnings classified as expected until drain
- focused cargo tests for the touched APIs
- verified bridge/readiness scripts, if present
- `nix flake check /home/brittonr/git/ucan -L` before archive
