## Context

Delegation verification proves authority exists, but application authorization also needs request facts and policy constraints. Caveats are the boundary where domain-specific constraints enter. Because the user values a modern reusable UCAN-like library over strict UCAN interop, this design should optimize for typed policy hooks, fail-closed behavior, and FCIS boundaries.

## Goals / Non-Goals

**Goals:** preserve caveat payloads, evaluate caveat attenuation through caller policy, verify concrete invocations, and fail closed by default.

**Non-Goals:** universal caveat language, revocation/replay storage, remote policy fetching, or UCAN-spec compatibility as a primary driver.

## Decisions

### 1. Keep caveat storage codec-neutral in core

**Choice:** `ucan-core` validates capability shape and preserves optional caveat payload references without understanding JSON or application semantics.

**Rationale:** Core must stay `no_std`; caveat interpretation belongs to deterministic policy logic supplied by the shell caller.

**Alternative:** Put `serde_json::Value` in core. Rejected because it pulls codec and allocation policy into the functional core.

**Implementation:** Add decoded caveat payload views/owned values shaped as bytes or shell-neutral data, then let `ucan` encode/decode compact-token caveats.

### 2. Use explicit caveat policy hooks

**Choice:** Add policy traits for child-vs-parent caveat attenuation and request-fact satisfaction.

**Rationale:** Caveats are domain-specific. The library should orchestrate when policy is required and fail closed when policy is missing.

**Alternative:** Treat caveats as opaque and always accept. Rejected because that silently drops security constraints.

**Implementation:** Provide a no-caveats policy for current behavior and require an explicit policy when caveats are present.

### 3. Model invocation as a first-class workflow

**Choice:** Add invocation request/result types that combine token verification, proof traversal, authorization matching, and caveat evaluation for one concrete operation.

**Rationale:** This is the app-facing workflow downstream crates actually need.

**Alternative:** Tell callers to manually verify, authorize, then evaluate caveats. Rejected because that creates inconsistent security pipelines.

**Implementation:** Build invocation on top of the signer/resolver/authorization APIs. Return structured allow/deny outcomes with matched capability and effective constraints.

## Risks / Trade-offs

**Scope can explode** → Keep this change to generic caveat hooks and invocation orchestration; defer revocation, replay storage, and domain-specific caveat libraries.

**Policy traits may need iteration** → Start with pure deterministic signatures so policy logic is easy to test without mocks.
