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

## Repeatability and limitations

Focused commands for the integration owner (not live qualification):

```console
cargo test --locked -p onix-kiln-core fair_admission --lib
cargo test --locked -p kiln-implementer --test admission
nickel export --format json config/fixtures/fair-admission-positive.ncl
nickel export --format json config/fixtures/fair-admission-weighted-positive.ncl
```

Each `config/fixtures/negative-fair-admission-*.ncl` export must fail. Behavioral
cases include competing campaigns and child bursts, duplicate receipts and quota
transfers, item/byte/fanout/group-active limits, weighted/dominant arithmetic and
priority ties, stale/incompatible offers, independent progress, shared model
allocation/request separation, cancellation races, authority/generation changes,
concurrent durable claims/reservation intents and reopen before/after possible
transport. Native Cairn proposal/design/tasks gates for `connect-fair-admission`
passed before product edits; those structural passes are not product test results.

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
