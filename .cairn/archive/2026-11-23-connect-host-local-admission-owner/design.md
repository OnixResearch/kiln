## Context

#61 landed bounded ready-work admission (`AdmissionLedger`), held owner debits,
recursive budget lineage and the `AdmissionOwnerPortV1` contract that
`admission_reserve_with_owner` drives. `kiln-lattice-dispatch` calls
`admission_before_transport`, which requires every queued owner's committed
reservation with a matching generation and a fresh eligibility observation.
Nothing outside the synthetic fixture helper produces those records, and
hand-written `reply` commands would let an operator assert a grant no owner
made. The operator `claim` command bypasses admission, so its claim is never
bound to the queue row.

## Goals / Non-Goals

Goals: a production path for one explicitly registered host-local owner that
reuses the existing ledger and owner contract; replay-safe, budget-preserving
commands the onix-core stage units can call; release only on an observed
terminal attempt; a governed operator claim; explicit per-issue intake
selection; an installed CI graph constructor.

Non-goals: observing or claiming remote, provider, model or serving capacity;
a new scheduler, queue or capacity database; automatic release, retry or
successor attempts; changing defaults without explicit policy or selection.

## Decisions

### Decision: a typed host-owner registration drives existing records

**Choice:** `HostOwnerProfile` (Nickel `config/fair-admission-host-owner.ncl`,
re-admitted by Rust) declares provider, host label, registration generation,
declared capacity, observation and reservation windows, campaign, tenant,
absolute deadline, finite quota, per-work amounts, the policy limits and the
admitted repositories with work classes. A work class is the exact `project`
and frozen `verification_profile` a work carries. The profile derives the
frozen `AdmissionPolicy` (dominant share, one campaign share, accounting basis
over provider, host and capacity dimension names), the one root account
(domain-tagged BLAKE3 identity) and queue rows that require this owner's
`execution` grant with `per_work` budget.

**Rationale:** the ledger already freezes policy identity and refuses a root
quota reset, so replay of the same profile is idempotent and a changed profile
is refused rather than resetting a budget.

### Decision: the owner decides from durable ledger facts

**Choice:** `plan_reservation` refuses unregistered work, a missing queue row, a
crossed claim, a stale/other-generation/non-eligible observation, a second
unreleased reservation for the same work and a request beyond capacity, and
otherwise plans one deterministic operation per claim (ordinal advanced only by
safely refused earlier operations). The shell persists it with the `request`
command, then the `HostLocalOwner` port reads the ledger again and `decide`
commits only if every other unreleased reservation of this owner plus this one
fits the declared capacity. Over capacity maps to `waiting`, a mismatch to a
before-dispatch rejection, an expired request to expiry.

**Rationale:** reading after the request is durable means two concurrent
requests see each other; a crash between request and reply is reconciled from
the same facts. A retained operation is returned without a new request.

### Decision: eligibility generation is the registration generation

**Choice:** eligibility observations carry the registration generation and are
valid for `observation_ttl_ms` (bounded by `max_observation_age_ms`); usage
observations advance their own generation with measured zero, so every retained
debit is composed on top. Bumping the registration generation on a capacity
change makes older observations and grants stale for dispatch.

### Decision: release only on observed termination

**Choice:** `plan_release` settles a granted reservation only when the attempt
ledger records its attempt/fence terminal, ended and not unresolved. Consumed
amounts equal the reservation; nothing is refunded. Time, restart or replay
never release.

### Decision: claim is governed under an explicit policy

**Choice:** with a policy retained, `kiln-implementer claim` runs the admission
`claim` command (retained eligibility, authority required, swarm deferral
refused). Without a policy the compatibility claim is unchanged.

### Decision: explicit intake selection

**Choice:** `ImplementationIssuePolicy.selection = { issues, labels }` is
optional; when present an open issue is admitted only when its number is listed
or it carries an exact label. Others become `IssueAdmission::Unselected`,
counted as `unselected` by polling and dropped by drain. Polled records and
webhook payloads carry bounded label names. An empty selection, issue zero or a
malformed label fails before any provider call. Source identities are unchanged.

### Decision: install the CI graph constructor

**Choice:** move the example's logic to
`Engine::construct_implementation_verification_service` beside its validator,
expose it as `kiln-web-service implementation-verification-graph` and delete
the example.

## Risks / Trade-offs

- Declared capacity is operator-reviewed, not probed; the owner is only as
  accurate as its registration, and it never reports anything else.
- A race-refused operation is final; the next reserve plans a fresh ordinal.
- Selection is evaluated at intake only; it never retracts admitted work.

## Verification Strategy

Core unit tests drive the committed Nickel export through the pure ledger:
admission, unregistered and class refusal, commit and dispatch intent, stale
and other-generation observations, capacity and a raced request, duplicate
grants and unobserved release. Implementer integration tests use the SQLite
store through `admission_before_transport`, restart and release. A CLI test
runs the operator commands and the governed claim. Poll and webhook tests cover
selected, unselected, exact-label, empty and malformed selections; the HTTP
transport test parses labels. The graph constructor has unit and installed
command tests.
