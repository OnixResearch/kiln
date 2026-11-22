# Opt-in bounded implementation admission

Issue #61 connects the existing implementation store and `operations::rank_tenants`
without replacing either. The current consumers are `kiln-implementer admission`,
`admission-status`, and the actual `kiln-lattice-dispatch` pre-transport and
acknowledgement paths. Kiln core/implementer maintainers own this boundary.
Inventory policy facts carry the same typed `AdmissionExplanation`; rendering it
cannot reserve anything.

## Enable deliberately

No configured admission policy means the existing configured compatibility path
is unchanged. A policy is immutable once retained in a selected implementation
store. Its BLAKE3 identity covers bounds, campaign shares, explicit weighted
resource dimensions, group limits and the accounting basis. There is no command
to disable a policy to escape a debit. Use a genuinely separate reviewed campaign
and accounting window, not a renamed queue or worker, for a new budget.

Human policy is `config/fair-admission-policy.ncl`. Positive dominant-share and
weighted fixtures and negative finite-bound fixtures are in `config/fixtures/`.
Export a reviewed input through that contract; wrap the exported policy as the
`configure` command. These operator commands consume explicit local files:

```console
kiln-implementer admission --database implementation.db --enrollment enrollment.json --command-file configure.json
kiln-implementer admission --database implementation.db --enrollment enrollment.json --command-file command.json
kiln-implementer admission-status --database implementation.db --enrollment enrollment.json
```

`command.json` is the strict versioned `AdmissionCommand` JSON representation,
with a snake-case `command` discriminator:

| Command | Data and effect |
| --- | --- |
| `configure` | `policy`; retain the explicit immutable opt-in policy |
| `root` | `account`; one finite quota root per admitted campaign |
| `transfer` | `parent`, `child`; current parent attempt/fence, inherited tenant/campaign/deadline and finite suballocation |
| `enqueue` | `item`; exact current admitted work, account, tenant/repository/campaign, deadline, aggregate budget, required owner grants and optional shared physical allocation references |
| `observe` | `observation`; complete/incomplete usage provenance, monotonic generation, exact accounting basis and settled receipt coverage |
| `select` | `inputs`; persist eligibility reasons and choose without making a claim or granting capacity |
| `claim` | `inputs`, `lease_ms`; atomically select and use the existing attempt state machine |
| `request` | `reservation`, `inputs`; persist a tentative exact operation before contacting its owner |
| `reply` | `reply`; admit the exact owner operation/generation/receipt and aggregate serving scope |
| `owner_failure` | exact operation/provider/generation, typed reason and bounded optional retry time |
| `cancel` | `work_version`, `expired`; stop future dispatch, retaining uncertain effects |
| `settle` | exact owner operation/provider/generation/receipt and actual `consumed` dimensions; definitive evidence only |
| `observe_serving` | `observation`; retain one serving-owner snapshot per aggregate scope; evidence, never a grant |
| `revoke` | `account`; revoke the account and its descendants without refunding held or unknown usage |
| `lineage` | `root`; return the conservation-checked usage of one root lineage |

The shell replaces item priority with the existing admitted work priority and
computes a conservative canonical payload byte bound itself, including future
claim/order/phase/reason metadata. It never trusts an input `bytes` value.
Array/file/state limits are independent of item counts. The ready queue is not
the authenticated GitHub intake `DeliveryQueue`.

Supply fresh host observations to the existing admitted Lattice invocation with:

```text
--admission-observations fresh-owner-observations.json
```

That file is a strict bounded array of `EligibilityInput`, with exact work
version, owner provider/generation, observed/expiry times and typed eligibility.
A display-time inventory explanation is not a fresh grant. Under an enabled
policy, unqueued work, missing grants, missing/ambiguous observations, revoked
enrollment, changed generation, expired deadlines or crossed claims block actual
transport. The strong candidate-producing worker route requires explicit policy;
its workspace/profile/deadline/effect admission remains independent.

<!-- r[impl kiln.admission.operator_claim] -->
Under an enabled policy, `kiln-implementer claim` selects and binds the queue
row through the same `claim` admission command, so the queue claim and the
attempt/fence agree before any owner request. It needs `--local-operator`, an
explicit opt-in guard naming the local operator, not authentication, and refuses
swarm deferral; it never claims around fair admission. Without a policy it keeps
the existing compatibility claim.

## Owner authority and backpressure

`AdmissionOwnerPortV1` has reserve, reconcile and cancel operations. The reusable
`admission_reserve_with_owner` shell persists the request transactionally before
calling the port. A retained operation uses reconciliation, including when two
callers race or the first caller crashes; it is never sent as an automatic new
reservation. The CLI can also consume independently acquired owner receipts.
Neither this trait nor manually importing a receipt invents a live provider.

`admission_cancel_with_owner` similarly commits the cancellation request before
calling the owner; re-entry reconciles rather than blindly repeating control.
Observed cancellation is shown as `cancelled`, while an unresolved usage debit
remains `unknown` until exact settlement. Neither control observation nor a late
reservation rejection refunds an unknown effect.

The existing `kiln-implementer cancel --database implementation.db --enrollment
enrollment.json --attempt ATTEMPT --fence FENCE` path atomically cancels the current
attempt and its exactly matching admission claim. An unqueued legacy claim is
still revocable under an enabled policy. This revocation is not proof that any
owner stopped a worker/request or released its physical reservation.

A worker inference request must bind finite `ServingBounds`: the owner's exact
aggregate scope, maximum ongoing requests and maximum queued requests across
callers. The owner reply must confirm those same bounds. A per-worker local
queue, idle-worker count or free-memory observation cannot supply that evidence.
The actual serving owner still enforces its queue; Kiln never implements an
inference router or loads/evicts/preempts a model.

Waiting, pre-dispatch rejection, expiry, backend failure, cancellation request,
observed cancellation and possible-effect unknown remain distinct. A host
retry-after must lie inside the policy's named finite bound. It is eligibility
time only: it creates neither an automatic retry, replacement semantic attempt,
model fallback nor budget reset. A cancellation observation alone does not
establish zero usage; authoritative settlement carries actual consumption.

### Serving-owner snapshots

<!-- r[impl kiln.admission.serving] -->
An optional `serving` policy block (`max_snapshot_age_ms`, `capacity_rule` of
`known_required` or `owner_admission_decides`) turns on a snapshot gate for
worker-request owners. Without the block nothing changes, including existing
policy identities. A `ServingObservation` is the provider-neutral projection of
one service entry of a host-supplied availability snapshot (Animus #17's
`animus.serving-availability-snapshot.v1` vocabulary): owner provider, owner
aggregate scope, source generation and sequence, deployment generation,
readiness, origin, observation window, the owner's declared `ServingBounds`,
and remaining ongoing slots and queue depth, where an unknown counter is never
zero. Kiln keeps one per scope; order cannot regress, a scope cannot change
owner, and the same order with different content is refused. Kiln does not
decode Animus JSON; the host shell projects it.

| Snapshot state | Outcome |
| --- | --- |
| none for the owner or scope | `incomplete` |
| older than `max_snapshot_age_ms`, from the future, past its window or source-marked stale | `stale` |
| `absent`, or bounds/owner differ from the request | `incompatible` |
| `unknown`, `loading`, `draining`, `unavailable` | `waiting` |
| host-probe counters or unknown slots under `known_required` | `incomplete` |
| no remaining slot and a full owner queue | `overloaded` |
| a remaining slot, owner queue room, or unknown slots under `owner_admission_decides` | eligible to ask the owner |

Selection applies the gate before ranking, so a saturated owner causes no claim
(pre-claim wait). A claimed item keeps its claim, attempt ordinal and lease
while it waits (post-claim wait). The `request` command applies the exact
scope/generation/bounds gate and returns the outcome without persisting a
tentative operation. A passing gate is not a reservation: two callers that see
the last slot both still need the owner's reply, and only an owner rejection
refunds. A retained owner operation is decided by the owner, not by a later
snapshot. Snapshots never refund, never reset a budget and never authorize a
different model or a new attempt.

## Registered host-local owner

<!-- r[impl kiln.admission.host_owner] -->
A live dispatch needs a policy, a root account, a queue row, a fresh owner
eligibility observation and a reservation committed by the resource owner.
For one explicitly registered host-local owner, Kiln produces all of them from a
reviewed profile, `config/fair-admission-host-owner.ncl` (positive fixture
`config/fixtures/fair-admission-host-owner-positive.ncl`, committed export
`fixtures/fair-admission/host-owner.json`). The profile declares the owner's
provider identity, host label, registration `generation`, declared `capacity`,
observation and reservation windows, the finite campaign `quota` and absolute
`deadline_ms`, the `per_work` reservation and the admitted repositories with
their work classes. A work class is the exact `project` and frozen
`verification_profile` an admitted work carries. Exporting the profile grants
nothing; these commands apply it through the ordinary ledger transitions:

```console
kiln-implementer host-owner-admit   --database DB --enrollment ENROLL --local-operator OP --profile host-owner.json
kiln-implementer host-owner-observe --database DB --enrollment ENROLL --local-operator OP --profile host-owner.json
kiln-implementer claim              --database DB --enrollment ENROLL --local-operator OP --lease-ms N
kiln-implementer host-owner-reserve --database DB --enrollment ENROLL --local-operator OP --profile host-owner.json --attempt ID --fence N
kiln-implementer host-owner-release --database DB --enrollment ENROLL --local-operator OP --profile host-owner.json --attempt ID --fence N
```

- `host-owner-admit` configures the derived policy (dominant share, one
  campaign share, accounting basis over provider, host and capacity dimension
  names), roots the campaign account and queues every current ready work whose
  repository and class are registered, each requiring this owner's `execution`
  grant. Every registered repository must be enrolled. Registered work the
  ledger does not queue is listed under `not_queued` with its reason
  (`queue_full`, `expired` or a lineage blocker). Other ready work, including
  unenrolled work, is listed as `refused` and never queued.
- `host-owner-observe` retains a complete usage observation (measured zero;
  every retained debit is composed on top, so nothing resets) and one
  eligibility observation per open queued work, valid for
  `observation_ttl_ms`, then runs the existing selection. Holders stay
  eligible. Only rows the selection could pick compete for the free registered
  slots: enrolled ready live claims first, then enrolled ready unclaimed work
  the attempt ledger would accept, each ordered as the selection orders them
  (priority descending, then enqueue order). Dead claims, de-enrolled, no
  longer ready and unclaimable work are observed as `waiting` without taking a
  slot, as is usable work beyond the free slots.
- `host-owner-reserve` plans one deterministic operation for the exact queued
  claim, persists the tentative request through the `request` command and asks
  the host owner through `AdmissionOwnerPortV1`. The owner decides from the
  ledger read after the request is durable: every other unreleased reservation
  of this owner occupies capacity. It refuses unregistered work, a crossed
  claim, a stale or other-generation observation, a second reservation for the
  same work and a request beyond capacity. A repeat or a restart returns the
  retained operation without a new request or debit. A tentative request left
  by an interrupted reserve is resolved first from its own record (committed,
  or rejected/expired with refund), so a crash never wedges the slot.
- `host-owner-release` settles the reservation only after the attempt ledger
  records the holding attempt terminal, ended and resolved. Whether the worker
  may still run comes only from the attempt ledger's transport facts: a
  recorded transport start or an acknowledged workflow. Lattice's no-effect
  witness (`settle_dispatch_without_execution`) clears the transport start, so
  a refusal before any effect, or a crash between the owner-grant gate and the
  transport start, releases once the attempt is terminal. While the worker may
  run, release also needs an observed stop: a reconciled termination
  (`reconcile-termination`), a retained candidate, or the worker observation
  Kiln retains only from Lattice's complete native result (`record_worker_observation`,
  including failed or aborted runs). The admission ledger's dispatch intent only
  decides consumption: with intent or transport the slot's budget is consumed;
  with neither (cancellation before dispatch) it returns unconsumed.

  **Unacknowledged runs.** A lost dispatcher or an unanswered exchange
  transport has a transport start but no workflow. On lease expiry `recover`
  marks it unknown; after `reconcile-termination` records the observed stop,
  `host-owner-release` frees and consumes the slot. A superseded attempt with
  started transport and no stop record is marked unknown and released the same
  way.

  **Held within this flow.** The only permanent hold is an acknowledged run (a
  recorded workflow) with no reconciled termination, candidate or retained
  worker observation whose lease expires: Kiln #45 does not mark it unknown, so
  `reconcile-termination` cannot bind it. That covers an acknowledged run whose
  native result never returned and one whose result returned but whose worker
  observation could not be retained (metrics unavailable). Elapsed time,
  restart and replay release nothing.

  **Starting host-owner work.** A claim of a host-owner queue row (its account
  is the domain-tagged host-owner campaign root) is bound in the attempt ledger,
  in the claim's own transaction, to require a recorded transport start before
  any workflow. `start_execution` and `reconcile_dispatch` (the `start` and
  `reconcile` commands, and every other workflow write) then refuse
  `implementation_transport_start_required` until `begin_dispatch_transport`
  has recorded the start, checked in the same attempt-ledger transaction as the
  workflow write. Only `kiln-lattice-dispatch` (worker request or workflow
  exchange) records that start, after the owner-grant gate and before any spawn
  or delivery. On both routes `--recover` only resumes an already persisted
  transport start: without one it is refused with "recovery requires prior
  persisted transport intent" before any write, so a fresh attempt always passes
  the owner-grant gate. Neither an out-of-band launch acknowledged later nor an
  ungranted claim can therefore adopt a worker. Attempts not claimed from a
  host-owner queue row are unbound, and `start` and `reconcile` are unchanged
  for them.

  **Claims must go through admission.** When the store's admission policy roots
  a host-owner campaign, every claim that bypasses admission (the library
  `claim_next` used by `kiln-fleet` and other embedders) is refused with
  `host_owner_admission_required`, decided in the same immediate transaction as
  the claim. The operator `claim` command and campaign claims already use
  admission. Without a host-owner root, every claim path is unchanged.

The owner's only capacity is its declared registration and its only occupancy
is the ledger's retained reservations for it. It never reports remote, provider
or serving capacity; a model gateway or serving owner remains a separate owner
with its own grant. Bump `generation` when the declared capacity changes:
observations and grants of another generation are stale, and dispatch refuses
them. A changed policy or quota is refused by the existing frozen-policy and
root-account checks, so a registration cannot reset a budget.

## Accounting and lineage

- Every reservation binds exact work, attempt/fence, operation, owner generation,
  resource/request subject and accounting basis. Only confirmed grants enter
  immediate held fairness debit; tentative requests still lock finite quota and
  active capacity while their outcome is uncertain.
- Existing `WeightedScalar`/`DominantShare` performs ranking, with unchanged
  arithmetic, freshness and stable campaign tie-breaks. Existing work eligibility,
  provider compatibility, finite budgets and group concurrency are checked first.
  Within a campaign, reviewed work priority precedes retained enqueue order.
- The accounting basis names one admitted dimension/normalization/window contract.
  Campaigns must report compatible capacity dimensions; incompatible amounts are
  not summed. Historical usage cannot regress, forget dimensions/receipt coverage,
  change its window or silently reset on reconnect. Partial pages make the campaign
  incomplete while retaining its prior measured amounts. Independent complete,
  compatible work can still advance.
  Fresh per-work eligibility is also retained across callers/restart. Omitting a
  known competitor from a later input cannot remove it from ranking; only a fresh
  owning observation, current work/authority change or expiry changes eligibility.
- Measured usage plus uncovered held/conservative operation debits determines
  fairness. Explicit exact *settled* operation/receipt coverage removes the
  corresponding conservative debit. Coverage cannot exceed reported measured
  consumption. Repeated grants, settlements, observations and quota transfers
  update one logical record and cannot mint/refund quota twice.
- Parent transfer removes quota from parent available and gives the child a finite
  allocation. It is not additional consumption. Original allocation identities
  remain retained after child usage. Child identities, queue names, reconnection
  and worker counts do not change the campaign root. Manager/coordination,
  execution and worker-request charges have explicit kinds under the lineage.
  A child account deadline must also fit the current parent work deadline and
  parent attempt lease, not merely a longer campaign account deadline.
- A physical model deployment has one physical subject/owner charge. Other workers
  may reference its confirmed same-tenant operation using `shared_allocations`;
  fresh generation and expiry are revalidated without another debit. Each worker
  retains its own request charges. Physical deployment lifetime remains the owner's
  responsibility, separate from acknowledging one worker workflow.

### Recursive lineage

<!-- r[impl kiln.admission.lineage] -->
Recursive workloads (for example a root model coordinator with generated
subcalls and nested child executions) charge one admitted lineage. Transfers
are subtractive, so new call IDs, workers, batches or nesting depth cannot
mint quota; the item budget and the account's available quota both bound every
request, and simultaneous children racing for the final allowance get one
success and one `admission_budget_exhausted` refusal before dispatch.

A lineage blocker walks an account's ancestors. A revoked account gives
`denied`; an ancestor deadline at or before now gives `expired`; a parent
attempt whose queued work was cancelled, had cancellation requested or expired
gives `cancelled` or `expired`. Enqueue returns the blocker without inserting,
selection records it, and request, transfer and dispatch intent refuse. A
parent that completes normally does not block its children. Revocation and
cancellation never refund outstanding or unknown operations.

`lineage_usage` (the `lineage` command) sums a root's whole subtree into
available, tentative, held, unknown, settled and rejected amounts, per-charge-
kind totals and the unknown-operation count. It fails closed unless the root's
original quota equals available + tentative + held + unknown + settled in every
dimension, so a small root transcript still reports every descendant's model
subcalls and unavailable accounting stays unknown. Explanations carry the
account's root, depth and blocker plus the serving gate and its snapshot.

`config/fair-admission-lineage.ncl` types a recursive budget profile: root
quota and deadline, named accounts with parent, allocation and deadline, and
finite depth and fan-out. It rejects forward or unknown parents, allocations
above the parent's remainder, deadlines past the parent's and depth or fan-out
beyond the bounds. Rust re-admits the export (`LineageProfile::validate`) and
derives domain-tagged BLAKE3 account identities from names. Exporting a profile
grants nothing: each transfer still needs a current parent attempt and fence.

## Effects, restart and cancellation

Preclaim waiting consumes no implementation attempt. After claiming, capacity
waiting preserves the same attempt/fence, consumed ordinal and lease. Expiry or
an exhausted attempt budget cannot be reclassified as preclaim queue time.

The extension uses the existing `implementation_state` SQLite table and immediate
transactions with work and attempt ledgers. Selection/order, quota debits,
reservation intent and dispatch intent are committed before possible effects.
After grants, the real dispatch path revalidates the current claim, enrollment,
work eligibility, policy identity, deadlines and every required fresh owner
reference. A lost reservation race is bounded waiting; it is not dispatch.
The actual worker route also bounds the request expiry by the queue deadline and
attempt lease, and the dispatch-plan runtime must fit inside the remaining queue
window. Unsupported sub-minute duration cannot be rounded up past the deadline.
The worker envelope's seconds/turns remain inside the admitted plan's runtime and
invocation bounds; a fresh preflight alone does not authorize a longer execution.

Possible transport changes the per-worker operations to `Unknown` before calling
the provider. The actual durable workflow acknowledgement projects `Executing`
without releasing accounting. A restart retains every debit/order/unknown and
requires reconciliation rather than blind redispatch. Cancelling or expiring a
queued item always makes it terminal for dispatch; late grants/acknowledgements
cannot revive it. An uncertain accepted request retains its debit and active slot
until the owner supplies definitive termination/usage settlement.

All retained items/accounts/operations and serialized state are bounded. Terminal
records are deliberately retained for identity/receipt safety, not silently
removed to admit more work. A full finite history fails closed with an explicit
outcome; this implementation does not perform shared GC or erase accounting
windows. Operator disposition for a new reviewed campaign/window is separate.

## Review-queue wait measurement

<!-- r[impl kiln.admission.queue_wait] -->
Before any admission, budget or backpressure change aimed at parallel-agent
review congestion, Kiln measures how long candidates wait in its own review
queue. The fixture is off by default: it runs only from the `review-queue-wait`
example and its integration test, never inside a service, and it changes no
admission, queue or budget default.

```console
nickel export --format json config/fixtures/review-queue-wait-positive.ncl
cargo run --locked -p kiln-web-service --example review-queue-wait -- \
  --load fixtures/review-queue-wait/load.json --out receipt.json
cargo test --locked -p kiln-web-service --test review_queue_wait
```

The load (`kiln.review-queue-wait.load.v1`, contract in
`config/review-queue-wait-contracts.ncl`) declares the following:

- a synthetic tick;
- a finite horizon;
- the queue bound;
- the queue modes;
- up to 16 candidates. Each has an arrival tick, a CI result (`pass`, `fail` or
  `unknown`) reported a set number of ticks after dispatch, and an optional
  submitter deadline.

The frozen load has eight candidates over 40 one-second ticks, with a queue
bound of seven entries. It runs serially and with a speculative window of one to
three. The Rust core admits the committed export again with the same bounds.
`scripts/check-nickel-fixtures.sh` refuses a stale export, and the negative
fixture must fail to export.

For each mode, the harness builds:

- a disposable bare Git repository with one source commit per candidate;
- the SQLite review store;
- the engine, which dispatches one CI execution for the project at a time;
- a scripted workflow provider.

In each tick it takes these steps, in order:

1. Enqueues the tick's arrivals with `enqueue`.
2. Withdraws entries past their deadline with `dequeue`.
3. Reports due CI results.
4. Polls the engine.
5. Advances the queue.
6. Reads `GET /api/code-review` and the candidates' runs.

Every time is the first tick boundary at which that state was visible, in
synthetic milliseconds since tick 0.

For each candidate, the receipt (`kiln.review-queue-wait.receipt.v1`) records:

- the exact source commit;
- every integration candidate and CI run;
- the arrival, enqueue, start, CI request and terminal times, where start is the
  first time the queue was seen constructing or testing the exact candidate;
- every dispatch of its CI to the workflow provider;
- its status transitions;
- one outcome from the table below.

| Outcome | Recorded signal |
| --- | --- |
| `submitted` | The queue submitted the exact candidate |
| `failed` | The queue blocked the entry with `review_queue_ci_failed` |
| `rejected` | `enqueue` was refused, for example with `review_queue_bound`, or the queue blocked the entry with another code |
| `expired` | The synthetic submitter withdrew the entry at its declared deadline; the review queue itself has no entry deadline |
| `cancelled` | Any other cancellation |
| `unknown` | The entry is `uncertain`, or its current CI run's outcome is unknown at the horizon |
| `waiting` | The entry is still active at the horizon |

A withdrawn entry's CI run that is still queued in the engine is dispatched
anyway. Its dispatch time can therefore follow its terminal time.

Review-queue wait, merge-queue wait, ready-work admission wait and inference
backpressure are separate typed series, and this fixture fills only the first.
The other three are `not_measured` with their reasons:

- merge-queue wait: no external merge or landing queue is in the path;
- ready-work admission wait: no implementation work is admitted;
- inference backpressure: no inference request is issued.

Those three cannot hold numbers. Reading a receipt back refuses:

- a series under another series' name;
- an edited summary;
- an outcome that does not follow its recorded signals;
- a load identity that does not match the embedded load.

Summaries count every outcome, zero included. They also give nearest-rank
distributions of the enqueue-to-start, enqueue-to-CI-dispatch and
enqueue-to-submit waits; an absent wait is never counted as zero.

Results describe queue mechanics under this declared load: scripted CI
durations, one CI slot and tick resolution. They are not production CI, reviewer
or merge latency. Two outside reports are context for why this is measured, not
loads, targets or pass criteria:

- Chen and Stratton's 14 November 2026 working paper,
  [Artificial Intelligence in the Firm: Bottlenecks in Software Production](https://fion.ac/jellyfish.pdf),
  reports a 56.1 percent rise in time from pull request submission to approval
  associated with agent adoption.
- Posters on X describe merge-queue stalls when several agents open pull
  requests at once.

A later admission, budget or backpressure change needs its own design, which
cites a retained receipt and names the measured bottleneck.

## Repeatability and limitations

Focused commands for the integration owner (not live qualification):

```console
cargo test --locked -p onix-kiln-core fair_admission --lib
cargo test --locked -p kiln-implementer --test admission --test admission_lineage
nickel export --format json config/fixtures/fair-admission-positive.ncl
nickel export --format json config/fixtures/fair-admission-weighted-positive.ncl
nickel export --format json config/fixtures/fair-admission-serving-positive.ncl
nickel export --format json config/fixtures/fair-admission-lineage-positive.ncl
```

The serving-policy and recursive-lineage exports are committed as
`fixtures/fair-admission/serving-policy.json` and
`fixtures/fair-admission/recursive-lineage.json`; the core and durable tests
read them and `scripts/check-nickel-fixtures.sh` refuses a stale export.

Each `config/fixtures/negative-fair-admission-*.ncl` export must fail. Behavioral
cases include competing campaigns and child bursts, duplicate receipts and quota
transfers, item/byte/fanout/group-active limits, weighted/dominant arithmetic and
priority ties, stale/incompatible offers, independent progress, shared model
allocation/request separation, cancellation races, authority/generation changes,
concurrent durable claims/reservation intents and reopen before/after possible
transport. Serving and lineage cases add saturated, stale, out-of-order,
host-probed and mismatched snapshots, two callers on the last slot, a two-level
recursive profile with changed call IDs, batches and depth, children racing for
the final allowance, duplicate requests and receipts, stale owner incarnations,
revocation, parent cancellation and expiry, and reopen with outstanding child
reservations. Native Cairn proposal/design/tasks gates for `connect-fair-admission`
and `connect-serving-lineage-admission` passed before product edits; those
structural passes are not product test results.

Fairness claims apply only to admitted comparable eligible finite workloads in
one explicit measurement window and the configured campaign shares. Reservations
are non-preemptible; arbitrary heterogeneous tasks have no strict latency,
completion-time or instantaneous-share guarantee. Accounting is not measured
billing. Missing usage remains conservative/unknown rather than zero.

A live claim additionally requires independently qualified Lattice #24 execution
bindings and #13 usage provenance, Aspen/Molten #42 exact grants/releases, Animus
#17 owner-aggregate admission/reconciliation and #49 authorized host assembly.
Retain exact producer cohorts, two CPU workers sharing one real model deployment,
competing campaign/child workload, actual capacity-change/rejection evidence and
crash/reopen/settlement receipts. These operations need operator-authorized owned
hosts and real provider implementations/credentials. No live fairness, serving,
physical placement, automatic preemption or production qualification is implied
by the local contracts, deterministic tests or offline operator view.
