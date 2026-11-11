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

### Operator commands

```console
nix run .#kiln-implementer -- drain --database /var/lib/kiln/work.sqlite \
  --queue /var/lib/kiln/github-issues.sqlite --maximum-deliveries 1024 \
  --secret-file /run/secrets/github-webhook \
  --policy /etc/kiln/implementer-policy.json \
  --enrollment /etc/kiln/implementer-enrollment.json --limit 64

kiln-implementer status --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/implementer-enrollment.json
```

`drain` re-verifies and admits queued deliveries; `status` reports retained work,
attempt and active counts. Both require absolute paths. The secret file is read
as exact bytes. Neither command claims or dispatches.

Author the policy and enrollment with the typed Nickel builders
`config/implementer-policy.ncl` and `config/implementer-enrollment.ncl` (see the
positive fixtures), then export JSON for the commands. The builders bound
acceptance obligations, paths, effects, budget, terminal policy, and unique
non-empty repositories/actors.

`run` repeats the drain on `--interval-ms`, stopping on SIGTERM/SIGINT or after
an optional positive `--iterations` bound. It reports the number of iterations
and drained deliveries. It never claims or dispatches; a scheduler that creates
attempts must wait for the Lattice implementation route.

`kiln-cairn-intake --database PATH --enrollment FILE --snapshot FILE --policy FILE --sequence N`
admits one complete `cairn.implementation-ready.v1` snapshot into the same
ledger, reporting selected/admitted/duplicate/superseded counts.

`claim` creates one current attempt (default lease one hour, `--lease-ms` to
override) and prints its canonical JSON including frozen work identity, base and
fence; `cancel --attempt ID --fence N [--cause TEXT]` revokes it.
`reserve --attempt ID --fence N --plan HEX --provider PROFILE` records the one
durable dispatch reservation before transport; repeating it is idempotent.
`reserve-plan --plan-file FILE` admits the exact JSON plan against the current
work and attempt, then records the same reservation from its canonical digest.
`start --attempt ID --fence N --workflow-kind KIND --workflow-id ID
[--namespace lattice]` records the provider workflow after an externally
observed dispatch and requires that reservation. Claims and reservations are an
operator/worker handoff only: no command performs transport, verification or
publication, and a claimed attempt must still be executed by an admitted Lattice
route.

## Ownership

| Concern | Owner | Status |
|---|---|---|
| Work admission, global claims, fences, reservations, budgets, disposition | Kiln | core and durable operator shell implemented |
| Versioned implementation-ready Cairn snapshots | Cairn `#1` | implemented; consumed by `kiln-cairn-intake` |
| One owned attempt workflow, worker route, cancellation | Lattice `#6–#8` | `#8` closed; `#6/#7` open |
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
