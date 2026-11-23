## Why

`bind-implementation-authority` (Kiln #69) landed the closed operation
vocabulary, the authority port, the Basalt adapter and enforcement at every
protected call site. No shell composes the verified profile, though: every
deployment selects `--local-operator` or nothing, replay and revocation exist
only as test doubles, payload binding is weak for publication, entry points
other than the nine enforcement sites are unclassified, and no test changes the
fence between an authority check and its commit.

## What Changes

- **C1 configuration.** A typed `kiln.implementation-authority-config.v1`
  (`config/implementation-authority.ncl`, accepting and rejecting fixtures) with
  the `absent`, `local_operator` and `verified_ucan` profiles. One helper,
  `Implementer::select_authority`, composes it at every shell: the GitHub
  implementer, `lattice-dispatch`, the Lattice worker, the fleet store, the web
  campaign drive and `verify-implementation`.
- **C2 payload v2.** `payload_digest_v2` binds the operation, the worker
  incarnation (the attempt's recorded transport launch, or `pre-launch`), the
  stable effect identity, the expected remote target state and the payload
  parts. Publication now binds the plan, not only the effect kind.
- **C3 durable state.** Kiln-owned SQLite replay, revocation-view and decision
  tables beside the ledger. Replay is insert-or-deny in its own immediate
  transaction; a stale revocation view denies; every decision is persisted.
- **Holder signing.** One holder key per role (coordinator, verifier,
  publisher) signs exact holder invocations; a role key cannot sign another
  role's operation.
- **C4 coverage v2.** `ENTRY_POINTS` classifies every `Implementer` public
  function, `kiln-implementer` CLI subcommand, fleet API route and web campaign
  control route. A source-scanning test fails when an entry point lacks a row.
- **Claim path.** `claim_next` and `claim_next_deferred` admit the claimed
  attempt as an exact `ClaimAcquire` invocation inside the claim transaction.
  They previously claimed without authority.
- **Dispatch path.** `reserve_dispatch`, `reserve_dispatch_plan`,
  `start_execution` and the fleet session bind admit the exact `Dispatch`
  request inside the transaction that saves their record, and the fleet tick
  rechecks it before starting the worker session. The GitHub CLI `reserve`,
  `reserve-plan` and `start` reach these checks. The Lattice worker admits the
  reservation's request before it reads its credential. They previously
  reserved and launched without a `Dispatch` decision.
- **Campaign roles.** The web campaign drive passes its worker, capture and
  verifier roles exactly the authority source it selected. They previously
  received none and always ran under the absent profile.
- **Reconciliation.** Adopting an observed dispatch is a fresh `Dispatch`
  invocation bound to the original reservation's effect identity.
- **Tests.** Verified positives and negatives for every operation in the
  adapter; fence-at-commit interleavings with two connections; an end-to-end
  test that selects the verified profile from configuration files.

## Impact

- **Core**: coverage schema v2, `ENTRY_POINTS`, `payload_digest_v2`,
  `incarnation_identity`, `AuthoritySelection`, `HolderInvocationSigner`.
- **Adapters**: `kiln-adapter-basalt` gains state ports with token reference,
  generation and clock, the role signer, composition helpers and a
  `test-support` feature.
- **Shells**: `kiln-implementer` gains `authority_config`, `authority_state`
  and the claim-path check; every composition site selects through one helper.
- **Configuration**: `config/implementation-authority.ncl`; optional
  `authority_config` in `config/fleet.ncl` and
  `config/implementation-campaign-drive.ncl`.
- **Not changed**: native-Git delivery and requests still carry only the local
  operator; remote conditional guarantees stay live-run evidence (#47/#48).
