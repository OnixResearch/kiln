## Context

`CaveatPolicy` remains a caller-supplied shell trait because UCAN caveat payload semantics are application-defined. The vendored `ucan_caveat` module has concrete finite helpers, but they are not an exact replacement for arbitrary shell policies.

## Goals / Non-Goals

**Goals:** expose typed caveat helpers for the subset of caveat semantics already modeled in vendored verified-logic, and make them usable by policy implementations.

**Non-Goals:** no automatic interpretation of opaque caveat payloads, no policy registry, no default allow-on-unknown behavior, no network/storage/time reads, and no replacement for `DenyUnknownCaveats`.

## Decisions

### 1. Helpers are opt-in policy building blocks

**Choice:** The verified helpers live beside other bridge helpers and are invoked explicitly by caller code or example policy implementations.

**Rationale:** This preserves the application-owned caveat semantics boundary.

**Alternative:** Automatically decode all caveats into vendored typed caveats during token verification. Rejected because arbitrary caveat payloads are intentionally opaque and unknown caveats must fail closed unless a caller policy accepts them.

## Risks / Trade-offs

**Confusing typed helpers with full caveat verification** → Public docs MUST state that shell verification still depends on explicit caller-supplied `CaveatPolicy` implementations.
