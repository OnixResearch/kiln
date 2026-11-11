## Why

The implementation-work contract and attempt state machine were pure. A
scheduler needs an atomic durable store so two processes cannot both claim one
work unit, and so restart preserves deduplication, budget, lineage and stale
fence refusal.

Issue: [#45](https://github.com/OnixResearch/kiln/issues/45), with the work
contract from [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `kiln-implementer`, a durable shell over `WorkLedger` and `AttemptLedger`.
- Store both ledgers as versioned rows in one SQLite database.
- Perform every mutation inside an immediate transaction, serializing
  concurrent claims.
- Expose admission, deterministic `claim_next`, lease renewal, execution start,
  dispatch reconciliation, candidate/verification transitions,
  failure/block/cancel/supersede, and `recover`.
- Preserve deduplication, consumed budget, attempt lineage and stale-fence
  refusal across close/reopen.

## Impact

- **Files**: `crates/kiln-implementer/**`, root workspace members,
  `docs/implementer-service.md`.
- **Testing**: two concurrent schedulers, duplicate delivery across restart,
  stale fence after expiry/replacement, dispatch reconciliation across restart,
  consumed budget after restart.
- **Authority**: Kiln owns durable claims, fences, budgets and disposition. The
  shell does not execute or cancel Lattice work, verify candidates, publish, or
  grant release authority.
