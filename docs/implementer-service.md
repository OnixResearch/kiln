# Implementer service boundary

This document records the ownership split and the exact remaining prerequisites
for automatic issue/Cairn implementation (issues #44–#50). It is a design
boundary, not an implemented service.

## Implemented in `kiln-core`

- `kiln.implementation-work.v1` admission: bounded contract, canonical identity
  and alias key, explicit issue↔task mappings, enrollment, duplicate/stale/
  ambiguous/superseded replay, dependency/acceptance blocking, cycle refusal,
  deterministic ready ranking (`crates/kiln-core/src/implementation/mod.rs`).
- `AttemptLedger`: one-current-claim admission, frozen work contract, monotonic
  fences, bounded leases and expiry, single-run dispatch reconciliation,
  persisted attempt states with distinct failure classes, abort/finish
  supersession, serialized lineage and budget
  (`crates/kiln-core/src/implementation/attempt.rs`).
- Authenticated GitHub `issues` intake producing one work record from operator
  policy plus exact delivery bytes, with versioned edits and an explicit closed
  observation (`crates/kiln-adapter-github/src/implementation.rs`).

## Implemented shell

`kiln-implementer` stores the work and attempt ledgers as versioned rows in one
SQLite database and performs every mutation in an immediate transaction. It
offers admission, deterministic `claim_next`, lease renewal, execution start,
dispatch reconciliation, candidate/verification transitions, failure/block/
cancel/supersede, and `recover`. Concurrent schedulers serialize on the store;
restart preserves deduplication, consumed budget, attempt lineage and stale-fence
refusal.

The shell owns durable claims and state, but still does not execute or cancel a
Lattice workflow, create a workspace, verify a candidate, publish, or grant
authority. Live dispatch remains blocked on the Lattice implementation route.

## Ownership

| Concern | Owner | Status |
|---|---|---|
| Work admission, global claims, fences, budgets, disposition | Kiln | core implemented; durable shell not wired |
| Versioned implementation-ready Cairn snapshots | Cairn `#1` | unimplemented in the Cairn repository |
| One owned attempt workflow, worker route, cancellation | Lattice `#6–#8` | open |
| Relocatable VM sessions | Lattice `#9–#11`, Aspen | open |
| Candidate acceptance using the authoritative Cairn checker | Kiln `#46`, Cairn | blocked by candidate producer |
| Forge publication and reconciliation | Kiln `#47` | blocked by #45–#46 |
| Deployment packaging | onix-core | repository absent locally |
| Live migration qualification | Kiln `#55`, physical hosts | unavailable |

## Required interfaces before dispatch

1. **Durable work/attempt store.** The serializable `WorkLedger` and
   `AttemptLedger` need a shell-owned durable store keyed by canonical work key
   and attempt identity, with atomic claim and fence increments. The existing
   journal/outbox contracts are the persistence model; no second workflow engine
   is needed.
2. **Lattice implementation route.** A separately admitted profile that accepts
   one frozen implementation contract (exact base, permitted paths/effects,
   deadline, token budget, expected output contract) and returns one workflow
   run. The review/agent-job route must not be widened.
3. **Worker result contract.** Candidate, logs, usage and evidence must carry
   the current attempt fence; stale fences are rejected before persistence.
4. **Cairn readiness intake.** Consume Cairn `#1`'s versioned snapshot only.
   Archive readiness stays separate; archived or deleted changes never prove
   completion.
5. **Acceptance.** Feed real retained execution results into Cairn's
   authoritative completion evaluator; schema-only validation is insufficient.
6. **Publication.** A guarded forge provider with independent read-back and
   reconciliation before any retry.

## Non-claims

- An admitted work record is intent, not execution permission or acceptance.
- A claim is not a completed attempt; a candidate is not verified.
- A successful worker invocation, edited task list, deleted directory or opened
  pull request is not landed completion.
- The current code grants no merge, release, or deployment authority.
