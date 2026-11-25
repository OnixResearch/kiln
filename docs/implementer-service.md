# Implementer service boundary

This document records the implemented ownership split and the exact remaining
qualification prerequisites for automatic issue/Cairn implementation. The
[operator runbook](implementation-runbook.md) contains the current native commands;
source implementation is not a live service or deployment qualification.

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

The ledger does not execute external work by itself. The separate
`kiln-lattice-dispatch` consumer binds and revalidates the admitted native Lattice
request before execution/recovery. The selected-Cairn controller composes that
consumer with independent candidate capture, canonical verification and delivery
owners. Live operation still requires a qualified immutable source/executable,
workspace, resource and model-admission cohort.

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
attempts must use the separately admitted Lattice implementation route rather
than treating intake or a claim as execution permission.

`kiln-cairn-intake --database PATH --enrollment FILE --snapshot FILE --policy FILE --sequence N`
admits one complete `cairn.implementation-ready.v1` snapshot into the same
ledger, reporting selected/admitted/duplicate/superseded counts. It is
available as `nix run .#kiln-cairn-intake -- ...`.

`poll --database PATH --enrollment FILE --policy FILE --secret-file FILE
--max-pages N [--since-ms N] [--timeout-ms N]` performs one authenticated GitHub
issue-polling recovery pass. `poll-run` performs the same pass on an operator
interval up to one day, with optional `--iterations` and SIGTERM/SIGINT
shutdown; it is not a daemon or cron replacement. When `--since-ms` is omitted,
`poll-run` starts from the loop start time instead of rescanning oldest history;
pass it explicitly for recovery. Polling uses the bounded HTTP transport and
admits normalized work into the ledger. Repeated polling is deduplicated by the work
ledger; closed issues are observed but not treated as completion. It does not
schedule itself or dispatch work.

`policy-check --policy FILE --enrollment FILE` is the intake preflight. It
admits the policy and enrollment with exactly the types, policy validation,
work construction and ledger admission `poll` applies, using one synthetic
open issue from an enrolled actor and an in-memory ledger, and prints
`{"policy":"admitted","repository":"owner/repo"}`. It takes no `--database`
and reads no credential. Webhook drain and polling apply the same policy
validation before any delivery or provider call. See the
[implementation runbook](implementation-runbook.md#intake-policy-and-immutable-preparation).

Every GitHub HTTPS client (status, polling, publication, landing and source
status) resolves `api.github.com` in a killed-and-reaped subprocess: it re-runs
the current executable as `<exe> --internal-github-status-dns <ppid>` with a
cleared environment and bounded output. `kiln-implementer`, `kiln-adapter-github`
and `kiln-hosted` serve that reserved entry before parsing their own arguments,
through `kiln_adapter_github::status::run_internal_resolver_if_requested`. A
binary that builds a GitHub client must call it first in `main`; otherwise client
construction fails with `github_status_dns_unserved` instead of re-running an
executable that cannot answer. A helper failure is reported on standard error
with the prefix `github status dns helper:`.

`claim` creates one current attempt (default lease one hour, `--lease-ms` to
override) and prints its canonical JSON including frozen work identity, base and
fence; `renew --attempt ID --fence N [--lease-ms N]` extends a live lease;
`cancel --attempt ID --fence N [--cause TEXT]` revokes it.
`reserve --attempt ID --fence N --plan HEX --provider PROFILE` records the one
durable dispatch reservation before transport; repeating it is idempotent.
`reserve-plan --plan-file FILE` admits the exact JSON plan against the current
work and attempt, then records the same reservation from its canonical digest.
`start --attempt ID --fence N --workflow-kind KIND --workflow-id ID
[--namespace lattice]` records the provider workflow after an externally
observed dispatch and requires that reservation. `reconcile --attempt ID --fence N
--workflow-kind KIND --workflow-id ID [--namespace lattice]` records the one
observed run for an ambiguous dispatch; an equal observation is idempotent and a
different one is refused. `handoff --handoff-file FILE`
admits a complete candidate handoff and moves the attempt to
`candidate-produced`; it does not verify or accept the candidate.
`metrics --attempt ID [--usage-file FILE] [--record true]` previews or retains
the default-off attempt metrics receipt; missing usage stays explicitly absent.
`kiln-web-service verify-implementation --profile CI_PROFILE --database DB
--enrollment FILE --attempt ID --fence N --verification-profile FILE --authority FILE`
executes the frozen checks under independent verifier authority, retains native
observations in canonical CI and admits the exact candidate result. The old
`kiln-implementer verify`/JSON-success import is removed.
`prepare-publication --publication-file FILE` prepares the deterministic verified
branch/PR plan; it creates no remote branch or pull request.
`prepare-stack-publication --stack-file FILE` (`{"plan": StackPlan,
"layer_verifications": [...]}`) prepares a durable 2–16 layer ordered stack
plan in separate `implementation_state` SQLite keys without changing
`AttemptLedger` or `PublicationRecord`.
`publish --publication-file FILE --secret-file FILE --remote-profile FILE [--route pull_request|github_stack|origin_stack] [--stack-file FILE] [--timeout-ms N]` pushes the exact captured candidate Git objects via the enrolled `NativeRemoteProfile`, verifies the exact remote branch, then creates/reconciles one matching PR (default `pull_request`), or probes `supports_stacks` (`github_stack`) and publishes via `publish_stack` or falls back to the prepared single-PR `--publication-file` on `404`; `origin_stack` refuses pre-effect (`implementation_stack_destination_unsupported`). `publish-stack --stack-file FILE --secret-file FILE --remote-profile FILE [--timeout-ms N]` runs or reconciles ordered stack publication directly. The strict `kiln.github-publication-source.v1` profile binds the canonical repository path, remote profile/identity, and explicit fixture discriminator. Production publication requires the exact HTTPS GitHub repository; local bare remotes are explicit loopback-fixture-only. It never force-pushes, updates an existing branch, merges or lands, and live GitHub stacked-PR endpoints remain unexercised.
`recover-integration --publication-file FILE --landing-plan FILE --secret-file FILE
[--timeout-ms N]` reconciles external integration by exact Git-object mapping,
current checks/reviews and final target ancestry. `recover-stack-integration
--stack-file FILE --secret-file FILE [--timeout-ms N]` performs the same
read-only recovery for ordered stack bottom prefixes, and `stack-gate
--stack-file FILE --landing-plans FILE --secret-file FILE [--timeout-ms N]`
prints bottom-up `LayerGate` states. Neither issues a merge request; ordinary
GitHub merging lacks the required atomic candidate-and-base guard.
`admission-explanation --work WORK_ID` reads `AdmissionLedger::explanation`
without mutating state or loading holder seeds.
`close-source --source-plan FILE --secret-file FILE [--timeout-ms N]` is
observation-only: it retains an already closed issue only at the plan's exact
admitted provider revision. It never PATCHes an open issue, accepts an arbitrary
new timestamp, edits other fields or performs Cairn lifecycle mutation.
Automatic source closure requires a separately qualified conditional-update
provider and remains unavailable.
`source-status --source-plan FILE --secret-file FILE [--timeout-ms N]
[--require-match true]` performs a read-only state comparison and prints
`retained`, `observed` and `match`; `--require-match true` exits non-zero on a
mismatch. It does not reopen, close or edit the issue.
Claim and reservation commands are an operator/worker handoff only. Independent
verification, publication and reconciliation use the separate native consumers
above; none turns a worker's exit status or self-reported checks into acceptance.
A claimed attempt still requires its exact admitted Lattice route and current
resource/model grants before execution.

## systemd deployment

`deployment/kiln-implementer-poll.service.in` runs the bounded `poll-run` loop
as a dedicated non-root account with a read-only system, an explicit state
directory, a root-private GitHub token credential, and one-day-capped polling
intervals. Provision `/etc/kiln-implementer/{enrollment,policy}.json` and the
credential file before installing the substituted unit. Stopping the unit sends
SIGTERM; the loop stops between polls without waiting out the full interval.

## Ownership

| Concern | Owner | Status |
|---|---|---|
| Work admission, global claims, fences, reservations, budgets, disposition | Kiln | core and durable operator shell implemented |
| Versioned implementation-ready Cairn snapshots | Cairn `#1` | implemented; consumed by `kiln-cairn-intake` |
| One owned attempt workflow, worker route, cancellation | Lattice `#6–#8` | native consumers implemented in the selected source; assembled cohort qualification remains separate |
| Relocatable VM sessions | Lattice `#9–#11`, Aspen | open |
| Candidate acceptance from independently executed canonical CI | Kiln `#46`, Cairn | native frozen-check and authoritative-completion consumers implemented; real-worker qualification remains separate |
| Forge publication, integration and source status | Kiln `#47/#48` | exact publication and read-back implemented; automatic GitHub landing and conditional source closure remain unqualified |
| Deployment packaging | onix-core | opt-in module and cohort/VM constructors implemented; device-free checks do not qualify a running worker |
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
