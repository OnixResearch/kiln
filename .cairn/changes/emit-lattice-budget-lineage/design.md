## Context

Lattice's `Binding` (`src/implementation_attempt/lineage.rs` at `5a8e1d51`) is
one frozen snapshot of the attempt's account: `root`, `account`, `parent`,
`depth`, `remaining`, `blocker` and `backpressure`. Lattice validates the
shape and refuses a blocker, backpressure, a zero remainder, or a charged
remainder below the first step as `lineage_blocked` or `lineage_exhausted`
before its fence and intent. It charges only `model_requests` and
`tool_invocations` and lists every other dimension as unaccounted. The
request binds the lineage through its exact bytes, not through its profile
revision.

Kiln's ledger roots each campaign in one account. A queue row names the
work's account, and an owner reservation is subtracted from that account's
`available` amounts when it is requested.

## Decisions

### Decision: the attempt's remainder is its committed reservations

**Choice:** `root` and `depth` come from `LineageExplanation`. `account` is
the queue row's account and `parent` is that account's parent. `remaining`
is the per-dimension sum of the attempt's `Committed` reservations, those
whose claim is this attempt and fence. Each amount must fit the work budget
and the root allocation. Every dimension the account tracks must have a
positive amount.

**Rationale:** after the debit, the account's `available` amounts are what
is left for everyone else, so they can be zero while the attempt holds its
grant. Unreserved quota belongs to no attempt. Sending it would let Lattice
draw quota that no owner has granted. If the lineage omitted a tracked
dimension, Lattice would be unbounded in it.

### Decision: append on the worker route; check on the exchange route

**Choice:** the worker route appends `,"lineage":{…}` before the request's
closing brace, keeping every admitted byte in place. The request seal and
the frozen file name stay keyed by the admitted bytes. The frozen file holds
the dispatched bytes. Recovery checks the frozen file against the admitted
bytes (`verify_frozen`) and never derives the lineage again. The exchange
route runs the same derivation but leaves its `lattice.workflow-request`
bytes unchanged.

**Rationale:** Lattice binds its state to the bytes it ran, so later
`recover`, `cancel` and `export-candidate` calls must name the frozen
request, and the campaign export now does. Recovery runs after the
dispatch intent has moved the grants to unknown, so deriving again would
fail or differ. Keeping the seal on the admitted bytes leaves the campaign's
retained-binding checks unchanged. The exchange contract denies unknown
fields, so it cannot carry the lineage.

### Decision: refuse inconsistency, never guess

**Choice:** `LineageRefusal` covers each case. A `Tentative` or `Unknown`
reservation is unsettled. A reservation on another account is crossed.
Other ledger contradictions are inconsistent. A blocker, a serving gate
other than `none`/`eligible`, or a held amount of zero in a tracked dimension
is refused. An identity Lattice cannot read is unrepresentable: Kiln accepts
uppercase hex and never lowercases it. A request that already carries a
lineage is refused. Each refusal happens before the owner-grant gate and the
transport start.

**Rationale:** Kiln's own dispatch intent already refuses a blocked lineage,
so emitting it would only move the refusal into Lattice.

## Risks / Trade-offs

- A recovery whose frozen request was lost is refused. Before this change it
  would recreate the file from the admitted bytes, which would cross
  Lattice's retained identity for a lineage-carrying attempt.
- Lattice's `lineage_blocked` and `lineage_exhausted` refusals are not in
  Kiln's settle-before-effect list, so Kiln retains them for reconciliation.
  This is conservative: Kiln's pre-transport checks make them unreachable
  for blockers and zero remainders.
