# Implementation work admission

Kiln can run exact candidate CI, but logged intent is not yet an admitted
implementation job. `kiln-core` now owns a bounded, versioned admission contract
that a future worker, Lattice attempt, and forge publisher can consume. It is a
boundary only: admission does not claim, execute, verify, publish, or complete
work.

## Contract

`ImplementationWork` binds:

- the canonical work identity and canonical alias key;
- exact source aliases and their source versions (`github` issue or `cairn`
  task), each a BLAKE3 of the bytes the adapter admitted;
- enrolled repository, project, and actor;
- title, requested outcome, and reviewed acceptance obligations;
- dependency identities, permitted repository-relative paths, and permitted
  effect classes;
- verification profile, attempt budget, terminal policy, and priority.

Identities are canonical BLAKE3 over a length-prefixed field encoding. The
canonical alias key excludes content, so versions of one source unit share a
key and are ordered by an adapter-supplied monotonic `revision`.

## Admission behavior

- A duplicate observation returns the retained identity; a lower revision is
  `stale`; an equal revision with different content is refused as ambiguous.
- A higher revision supersedes the retained version. Both versions remain
  auditable; the old snapshot is never rewritten.
- An explicit `SourceMapping` links one issue to one or more task-level units.
  Every mapped alias must appear in the admitted aliases; duplicate or forged
  mappings, unrelated same-title work, and partial mappings are refused.
- Missing dependencies, empty acceptance obligations, and dependency cycles are
  visible `blocked` reasons. Blocked work is never ranked.
- Only ready work is ranked, by reviewed priority and then canonical key.
- Unenrolled repositories or actors, unknown schemas, unsafe paths, unknown
  effects, missing budgets, and oversized fields fail admission before any
  version is retained.

Archive readiness is deliberately absent. An implementation-ready Cairn task
can be admitted while archive readiness is false, and an archived or deleted
change never proves implementation completion.

## Implementation attempts

`kiln-core` also owns the global claim over admitted work. `AttemptLedger`
admits at most one current attempt per canonical work key, freezes the work
identity/revision/ordinal and reviewed budget, and assigns a monotonic fence.
Every candidate and later request must present the current unexpired fence; a
stale worker cannot register or publish after a lease expires and a replacement
claims the work.

Claims carry bounded leases. Expiry blocks the attempt with `lease_expired`,
revokes its fence and consumes the budget; a replacement may claim only while
the lineage still has attempts. Dispatch reconciliation accepts exactly one
observed workflow run: an absent record is filled, an equal observation is
idempotent, and a conflicting run is refused instead of redispatched.

States are persisted with structured causes: claimed, executing,
candidate-produced, verifying, blocked, failed, cancelled and superseded.
Implementation, infrastructure and unknown failure classes stay distinct.
Supersession applies `AbortActive` (revoke the attempt) or `FinishActive` (let it
finish and record the newer revision) without mutating the frozen contract. The
ledger serializes, so attempt lineage and consumed budget survive restart.

This is the Kiln-owned state machine only. It does not execute a Lattice
workflow, cancel a run, create a workspace, verify a candidate, or publish. The
Lattice implementation route and worker contract remain external work.

## GitHub issue intake

`kiln-adapter-github` admits authenticated `issues` deliveries through
`admit_implementation_issue`. It bounds the body, validates delivery and event
headers, verifies the exact bytes with HMAC-SHA256, enforces the configured
repository and enrolled sender, and parses the strict UTC `updated_at` sequence.
Opened, reopened and edited issues produce one contract record whose source
version is the BLAKE3 of the delivery bytes; closing produces an explicit closed
observation. Acceptance obligations, outcomes, paths, effects, verification
profile, budget, terminal policy and priority come from operator policy, not
issue text. Duplicate and out-of-order deliveries are resolved by `WorkLedger`.

The HTTP server is not yet wired to this function, and bounded polling recovery
is not implemented.

## What is not implemented yet

The adapters that produce these records are separate work:

- GitHub polling recovery and server wiring. The admission function above is
  implemented; the long-running webhook service does not yet persist or poll it.
- Cairn intake. It must consume Cairn `OnixResearch/cairn#1`'s versioned
  implementation-ready snapshot; that producer is unimplemented. This contract
  intentionally does not fork Cairn lifecycle or readiness semantics.
- The Mneme OpenSpec consumer qualification.
- Claims, execution, candidate acceptance, and publication (issues #45–#48).

Until those land, this module is a pure, tested boundary with no dispatch
authority.
