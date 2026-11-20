## Context

`fair_admission::AdmissionLedger` already ranks eligible queued work with
`operations::rank_tenants`, bounds queues/bytes/fan-out/active work, retains
owner operations by exact operation/provider/generation, charges confirmed
grants immediately, and moves quota from parent to child accounts. The
implementer shell persists every transition in the existing
`implementation_state` transaction before owner calls. Serving bounds are only
declared on a worker request; there is no observation of the owner's actual
aggregate load, no revocation, and child accounts stay usable after their parent
attempt is cancelled.

## Goals / Non-Goals

Goals: serving backpressure from injected owner snapshots, pre-claim versus
post-claim serving waits, lineage revocation and cancellation propagation, a
lineage-wide usage view that proves no quota is minted, typed Nickel profiles
with checked generated exports, and policy explanations.

Non-goals: decoding Animus JSON (a shell/adapter concern), an inference router,
physical placement, preemption, a scheduler service, or live qualification.

## Decisions

### Decision: provider-neutral serving observation, opt-in by policy

**Choice:** `ServingObservation` mirrors one service entry of
`animus.serving-availability-snapshot.v1` plus its header: provider (serving
owner profile), owner aggregate scope, source generation and sequence,
deployment generation, readiness (`absent`, `unknown`, `loading`, `ready`,
`draining`, `unavailable`, `stale`), origin (`serving_owner`, `provider_api`,
`host_probe`), observation window, the owner's declared `ServingBounds`, and
optional remaining slots and queue depth. One record per scope is retained;
`(source_generation, sequence)` cannot regress, a scope cannot change owner, and
a repeated pair with different content is ambiguous. A new optional
`AdmissionPolicy.serving` block (`max_snapshot_age_ms`, `capacity_rule` of
`known_required` or `owner_admission_decides`) enables the gate. Absent block:
existing behaviour and existing policy identities are unchanged.

**Rationale:** Kiln core must not import provider types. The Animus vocabulary
keeps the shell projection mechanical.

### Decision: the snapshot gates waiting, the owner grant gates dispatch

**Choice:** the gate is evaluated per worker-request owner. Stale (too old,
future, expired window or source-marked stale) yields `stale`. Missing yields
`incomplete`. `absent` or bounds that differ from the reservation yield
`incompatible`. Not ready yields `waiting`. A host probe never counts as
capacity; unknown counters yield `incomplete` under `known_required` and pass to
the owner under `owner_admission_decides`. No remaining slot and a full owner
queue yields the new `overloaded` reason. Selection applies the gate before
ranking, so a saturated owner does not cause a claim (pre-claim wait) and a
claimed item keeps its claim, attempt ordinal and lease (post-claim wait). A
request applies the exact scope/generation gate before persisting a tentative
operation. A passing gate never reserves, never skips the owner reply, and two
callers seeing the last slot both still need the owner's grant.

### Decision: lineage blockers, revocation and conservation

**Choice:** `revoke_account` marks an account and every descendant revoked;
idempotent, no refund, outstanding/unknown operations retained. A lineage
blocker walks ancestors (bounded by retained accounts): revoked gives `denied`,
an ancestor deadline at or before now gives `expired`, and a parent attempt
whose queued work is terminal as cancelled, cancellation-requested or expired
gives `cancelled`/`expired`. Enqueue returns the blocker without inserting;
selection marks and skips; request, transfer and dispatch intent refuse.
`lineage_usage(root)` sums the subtree into available, tentative, held,
unknown, settled, rejected, per-charge-kind and unknown-operation counts and
fails closed unless the root's original quota equals available + tentative +
held + unknown + settled in every dimension.

**Rationale:** transfers are subtractive, so new child, call, worker or nesting
identities cannot mint quota; the conservation check makes that observable.

### Decision: typed Nickel profiles with committed exports

**Choice:** `config/fair-admission-policy.ncl` gains the optional `serving`
block. `config/fair-admission-lineage.ncl` types a recursive lineage profile
(root quota/deadline, named children with parent, allocation and deadline, a
finite depth bound) and checks per-parent allocation sums, deadlines, depth,
unique names and fan-out. Positive fixtures export to
`fixtures/fair-admission/serving-policy.json` and
`fixtures/fair-admission/recursive-lineage.json`;
`scripts/check-nickel-fixtures.sh` refuses stale exports; Rust tests read both.

## Risks / Trade-offs

- Snapshot freshness is caller-supplied time; stale or missing snapshots block
  rather than admit.
- Revocation is permanent for the ledger's window; a new budget requires a new
  reviewed campaign/window.
- Blocking only on cancelled/expired parents lets completed parents' children
  finish; callers that need stricter coupling must cancel the parent.

## Verification Strategy

Core unit tests for each gate outcome, monotonic snapshots, last-slot owner
decision, revocation, cancellation propagation, conservation, simultaneous
children, duplicate receipts and changed call identities. Implementer
integration tests drive the Nickel-exported recursive profile through the
SQLite store across reopen, stale-incarnation completions and settlement.
