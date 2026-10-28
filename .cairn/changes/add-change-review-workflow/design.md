## Context

The console has read-only Patchgate receipts, exact local source views, generic work COBs and candidate-bound gating primitives. None is an interactive change review. This change adds review semantics without replacing generic work or promoting receipt verdicts into authority.

## Goals and Success Contract

A user can create a stable change, upload exact successive revisions, compare them, privately draft and publish anchored discussions and votes, request review, find attention and notification items, browse dependency stacks and understand each blocked submit requirement. An authorized submit rechecks the exact candidate and target and reconciles effects durably. Author maintenance actions use the same explicit provider boundary.

## Decisions

### Decision: Pure review transitions and versioned provider ports

**Choice:** Put provider-neutral review state, admission, votes, dependency and checklist decisions in a pure module. Keep repositories, SQLite, clocks, credentials and external calls in shells behind versioned review-provider types. Reuse the existing Git library for a separately configured local Git collaboration adapter, not the read-only source adapter. Other providers reject unsupported mutation capabilities before effects.

**Rationale:** Exact immutable revisions and observed target CAS are the safe initial concrete implementation. A local Git adapter does not claim remote publication or Radicle collaboration behavior.

### Decision: Durable private application review state

**Choice:** Use a bounded service-owned SQLite store with transaction/CAS request admission, principal-bound idempotency and private draft/saved-view projection. Persist intended external operations before dispatch; retain uncertain outcomes and reconcile exact provider observations before a retry. Published threads, votes, reviewer roles, attention and notifications remain revision-bound. New revisions invalidate votes and file markers rather than silently carrying authority.

**Rationale:** Reuse the shell's SQLite conventions and authenticated principal; private drafts cannot be mixed with public generic work records or Patchgate receipt listing.

### Decision: Explicit provider and approval policy

**Choice:** Typed Nickel declares source repositories, writable collaboration targets, actors, reviewers, submitters, required approvals and CI requirements. Checklists expose missing/stale revision evidence, vetoes, unresolved discussions, WIP, dependency readiness and target movement. Required CI results are read from canonical engine state, never supplied by the browser. Submitting a dependent stack preserves order and re-evaluates each exact candidate after target movement.

**Rationale:** Green CI, owner votes, retained receipt decisions and stale approvals never confer submit permission. Browser actions confirm exact target and operation values immediately before effects.

## Risks / Trade-offs

- Provider success followed by a shell crash requires retained operation identities and exact read-back, not blind retry.
- Draft content and private saved queries must be excluded from other principals' responses and notifications.
- Diffs and line anchors must bind both selected revisions, exact file paths, side and valid ranges; binary and truncated files cannot pretend to be reviewable text.
- New revisions invalidate approval authority; explicit conservative behavior is preferable to implicit vote carry-forward.
- Local provider mutation does not grant remote publication authority; configured unsupported providers fail before effects.

## Verification Strategy

Exercise valid and malformed/stale/replayed commands, ownership and eligibility, exact source/target races, private projections, draft batch atomicity, dependency cycles/order, timeout/uncertain reconciliation and restart. Drive the real authenticated browser surface at desktop and mobile sizes against temporary repositories. Run required repository validation after integration, not during concurrent edits.
