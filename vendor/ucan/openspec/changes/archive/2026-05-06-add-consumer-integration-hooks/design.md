## Context

The library currently exposes bounded token issuance, parsing, verification, proof-chain, revocation, caveat, signer, and readiness surfaces. It is explicitly `controlled-integration-ready`, not a turnkey production platform. Aspen and Clankers both need to consume the same generic UCAN core while keeping their own authorization semantics in their own repositories.

## Goals / Non-Goals

**Goals:**
- Provide minimal generic hook traits for consumer-owned resolver/proof/revocation/replay infrastructure.
- Keep all consumer-specific policy mapping outside `ucan`.
- Make hook failure behavior structured, deterministic, and fail-closed.
- Add conformance tests with fake implementations so Aspen and Clankers can copy the integration pattern safely.
- Keep readiness labels conservative until concrete backend evidence exists.

**Non-Goals:**
- No Aspen adapter, Clankers adapter, or consumer-specific capability vocabulary in `ucan`.
- No built-in remote DID resolver, proof network transport, database registry, replay store, or key-management service.
- No production-supported promotion of a concrete backend solely because a trait exists.

## Decisions

### 1. Traits are generic infrastructure seams

**Choice:** Add public traits such as resolver, proof loader, revocation checker, and replay admission using UCAN-native inputs/outputs and structured errors.

**Rationale:** Multiple consumers need the same extension points but own different persistence, transport, identity, and policy implementations.

**Alternative:** Build concrete Aspen or Clankers adapters in `ucan`. Rejected because that would couple the library to application semantics and violate the consumer boundary.

**Implementation:** Traits live in the std shell unless a pure subset is needed in `ucan-core`. They delegate semantic checks back into existing token/proof/verification code.

### 2. Conformance before backend support

**Choice:** Add fake/in-memory implementations only for tests/examples and require positive plus negative conformance cases.

**Rationale:** The library can prove the trait contract without implying any real storage or network backend is production-ready.

**Alternative:** Add an optional concrete backend feature first. Rejected until a consumer demonstrates a reusable backend requirement.

### 3. Readiness status remains evidence-backed

**Choice:** Hook traits may be `controlled-integration-ready` while concrete remote/storage-backed surfaces remain below `production-supported` until separately proven.

**Rationale:** This preserves the anti-overclaiming invariant added by the production-readiness gates.

## Risks / Trade-offs

**Trait overfitting** → Keep trait methods small and UCAN-native; avoid Aspen/Clankers nouns and add conformance tests rather than consumer logic.

**Async/runtime mismatch** → Prefer a minimal synchronous trait shape first unless implementation inspection proves async is required; if async is needed, hide it behind feature-gated adapters rather than forcing a runtime.

**Error taxonomy drift** → Define structured error categories that map to fail-closed verification outcomes and are safe to expose without compact token or key material.

## Validation Plan

- `openspec validate add-consumer-integration-hooks --strict --json`
- helper verification, treating incomplete implementation tasks as expected until drain
- `python scripts/check-production-readiness.py`
- targeted Rust tests for hook conformance and fail-closed behavior
- `cargo test -p ucan --lib`
- clippy with warnings denied
- `nix flake check /home/brittonr/git/ucan -L`
- archive only after implementation evidence is captured and all tasks are complete
