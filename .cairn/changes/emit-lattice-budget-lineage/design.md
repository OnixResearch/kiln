## Context

Lattice's `Binding` (`src/implementation_attempt/lineage.rs` at `5a8e1d51`) is
one frozen snapshot of the attempt's account: `root`, `account`, `parent`,
`depth`, `remaining`, `blocker` and `backpressure`. Lattice validates the
shape and refuses a blocker, backpressure, a zero remainder, or a charged
remainder below the first step as `lineage_blocked` or `lineage_exhausted`
before its fence and intent. It charges only `model_requests` and
`tool_invocations` and lists every other dimension as unaccounted. The
request binds the lineage through its exact bytes, not through its profile
revision, and Lattice's intent names the BLAKE3 of the bytes it ran.

Kiln's ledger roots each campaign in one account. A queue row names the
work's account and the owners whose grants it requires, and an owner
reservation is subtracted from that account's `available` amounts when it is
requested.

## Decisions

### Decision: the attempt's remainder is its committed reservations

**Choice:** `root` and `depth` come from `LineageExplanation`. `account` is
the queue row's account and `parent` is that account's parent. `remaining`
is the per-dimension sum of the attempt's `Committed` reservations, those
whose claim is this attempt and fence, and every owner the queue row
requires must have committed one, as the owner-grant gate requires. Each
amount must fit the work budget and the root allocation. Every dimension
Lattice charges that the account tracks must have a positive amount; a
dimension Lattice cannot charge that the attempt does not hold is omitted.

**Rationale:** after the debit, the account's `available` amounts are what
is left for everyone else, so they can be zero while the attempt holds its
grant. Unreserved quota belongs to no attempt. Sending it would let Lattice
draw quota that no owner has granted. An omitted charged dimension would
leave Lattice unbounded in it, but Lattice never draws any other dimension,
so requiring one would only refuse work whose budget covers part of its
account, such as a worker reaching a shared deployment's `gpu` through
`shared_allocations`. Requiring every owner's grant keeps the remainder from
being read off part of the attempt's grants.

### Decision: freeze the sent bytes under their own digest and record them

**Choice:** the worker route appends `,"lineage":{…}` before the request's
closing brace, keeping every admitted byte in place, and freezes the sent
bytes as `request-<BLAKE3 of the sent bytes>.json` before the owner-grant
gate. The request seal stays on the admitted bytes. `begin_dispatch_transport`
records the frozen file's path and BLAKE3 in `TransportLaunch.request`.
Recovery and campaign export read exactly the recorded file. Recovery checks
that the file still hashes to the recorded BLAKE3 and is the admitted bytes
plus at most one well-formed lineage (`verify_frozen`), and never derives
the lineage again. A launch recorded without `request` predates this change
and sent the admitted bytes. The exchange route runs the same derivation but
leaves its `lattice.workflow-request` bytes unchanged.

**Rationale:** Lattice binds its state to the bytes it ran, so `recover`,
`cancel` and `export-candidate` must name the sent file. Naming it by its own
digest lets a retry refused before any transport start freeze new bytes
when its lineage changed (an upgrade, or a second owner's grant arriving)
instead of being refused forever by an earlier file. Recording it with the
launch makes recovery independent of the current profile's
`output_directory`, as the recorded program and state root already are. The
grants are unknown after the gate, so deriving again would fail or differ.
The exchange contract denies unknown fields, so it cannot carry the lineage.

### Decision: refuse inconsistency, never guess

**Choice:** `LineageRefusal` covers each case. A `Tentative` or `Unknown`
reservation is unsettled. A reservation on another account is crossed. A
required owner without a committed grant is `missing_owner_grant`. Other
ledger contradictions are inconsistent, each with its own detail. A
blocker, a serving gate other than `none`/`eligible`, or a held amount of
zero in a charged tracked dimension is refused. An identity Lattice cannot
read is unrepresentable: Kiln accepts uppercase hex and never lowercases
it. A request that already carries a lineage is refused. Each refusal
happens before the owner-grant gate and the transport start. Recovery
refusals say Lattice was not invoked for recovery and the first launch may
have run.

**Rationale:** Kiln's own dispatch intent already refuses a blocked lineage,
so emitting it would only move the refusal into Lattice. Root, account,
parent and depth come from one explanation walk, so they cannot disagree
and are not checked again.

## Risks / Trade-offs

- An older Kiln binary cannot read a launch record that names its request
  (`deny_unknown_fields`), so a rolling downgrade fails closed on such
  attempts.
- A recovery whose recorded frozen file was lost is refused and retained for
  reconciliation, never re-frozen from the admitted bytes.
- Lattice's `lineage_blocked` and `lineage_exhausted` refusals are not in
  Kiln's settle-before-effect list, so Kiln retains them for reconciliation.
  This is conservative: Kiln's pre-transport checks make them unreachable
  for blockers and zero remainders.
