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
- the exact resolved target/base revision when the producer has one (absence is
  explicit, not an inferred branch);
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
the lineage still has attempts.

Before transport, an attempt reserves exactly one dispatch binding: provider
profile, admitted plan identity, deterministic effect identity and idempotency
key. The intake policy MAY supply an exact resolved `base`; dispatch requires it.
A dispatch plan binds the work identity/revision, attempt/fence,
repository/base, isolated workspace, provider profile, admitted paths/effects,
cancellation-only controls, deadline, token/invocation budgets and expected
output contract. Work must admit every path and effect, the deadline must fit
the attempt budget, and network or credential access is refused. Repeating the same reservation is idempotent; a different plan or profile
for the live attempt, a stale fence, or an expired lease is refused. The
reservation survives restart. Dispatch acknowledgement and reconciliation
require the reservation, fill the single observed workflow run, accept an equal
observation idempotently, and refuse a conflicting run instead of redispatching.

States are persisted with structured causes: claimed, executing,
candidate-produced, verifying, verified, blocked, failed, cancelled and
superseded.
Implementation, infrastructure and unknown failure classes stay distinct.
Supersession applies `AbortActive` (revoke the attempt) or `FinishActive` (let it
finish and record the newer revision) without mutating the frozen contract. The
ledger serializes, so attempt lineage and consumed budget survive restart.

This is the Kiln-owned state machine only. It does not execute a Lattice
workflow, cancel a run, create a workspace, verify a candidate, or publish. The
Lattice implementation route and worker contract remain external work.

## Verified-PR publication

`PublicationPlan` prepares delivery only for a terminal `verified` attempt. It
binds the exact candidate handoff and verification receipt, work/version,
attempt/fence, repository, frozen base, target base branch, candidate/tree, a
deterministic `kiln/impl-<work>-<attempt>` branch, and bounded PR title/body. The work must use
the verified-pull-request terminal policy. Preparation performs no remote
effect.

`PublicationRecord` then records observed branch and pull-request identities in
order. The branch head and PR head must remain the exact verified candidate; a
different or out-of-order observation is refused. Remote publication itself is a separate provider effect. `IntegrationReceipt`
records the independently read-back merge commit and base head for an exact
published PR, moving the publication record to `integrated`. Source-status
reconciliation is a separate effect: a `SourceStatusPlan` may close the exact
GitHub issue only after an integrated publication, and a `SourceStatusReceipt`
retains the observed closed state.

## Candidate verification

`CandidateVerification` admits a retained canonical `CiFact` for the exact
candidate handoff. It binds the handoff identity, work/version, attempt/fence,
candidate and tree, base, verification profile and the exact run revision.
Admission requires a terminal, successful, clean full run with no untrusted
projection and nonempty unique checks that all pass. Repeating an equal receipt
is idempotent; partial, failed, stale or differently bound evidence is refused.
Acceptance moves the attempt to the terminal `verified` state; it does not
execute checks, create CI facts or publish anything.

`kiln-implementer verify --verification-file FILE` records the receipt against
the retained handoff and current work/attempt.

## Candidate handoff

`kiln-core` admits one complete candidate handoff for an executing attempt
before any verification. It binds the exact work identity/revision,
attempt/fence, repository and frozen base, candidate commit and tree, sorted
changed paths, the digest of the work's acceptance obligations, the verification
profile, toolchain and environment identities, and retained evidence
references. Every changed path must be inside a work-permitted path, evidence
identities must be unique, and candidate and tree must differ. Repeating the same
handoff is idempotent; a different handoff is refused.

`kiln-implementer handoff --handoff-file FILE` admits the handoff and moves the
attempt to `candidate-produced`. This is an envelope check only: evidence
outcomes are observations, the worker is not trusted, and an admitted handoff is
not an accepted candidate or a passing check. The independent verifier and Cairn
authoritative completion evaluation remain separate work.

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

The HTTP server is not yet wired to admit these functions directly. When
`implementation_queue` is configured, the GitHub webhook service authenticates
`issues` deliveries and stores their exact bytes in a bounded durable queue
(`kiln_implementer::DeliveryQueue`) for a separate scheduler; repeated delivery
IDs are idempotent, and a full queue refuses rather than evicting. Absence of the
queue keeps all prior behavior.

Bounded polling recovery is implemented behind `IssuePollTransport`:
`poll_implementation_issues` limits pages and records, enforces enrollment and
text bounds, selects the highest update sequence for duplicate records, orders
observations deterministically, and fails rather than truncating when more
records remain beyond the page bound.

`GithubIssuePollClient` supplies the authenticated transport: fixed
`api.github.com` origin, bearer token, no redirects or environment proxy,
bounded response bytes, pull-request filtering, strict timestamps and a bounded
`Link` next-page parser. Authentication, repository and rate-limit failures are
distinct; `kiln-implementer poll` runs one explicit bounded pass and retries and
the polling schedule remain separate.

`drain_implementation_queue` consumes a bounded queue batch: it re-verifies each
stored signature, normalizes through operator policy, admits into the durable
`kiln-implementer` ledger, and removes the record only after a terminal decision.
A forged or unverifiable record fails the drain and stays queued. Scheduling and
claim/dispatch remain operator shells.

## Cairn intake

`kiln-adapter-cairn` consumes `cairn.implementation-ready.v1` read-only
snapshots. `admit_cairn_snapshot` accepts only `complete: true` snapshots and a
caller-supplied monotonic sequence. A `ready` selection becomes one work record
per stable `task:<id>` or legacy `change:<name>` unit: the alias carries the
change/unit identity and the change snapshot digest as its source version, and
the unit's requirement references become explicit acceptance obligations beside
the operator policy's own obligations.

Non-ready outcomes (`blocked`, `empty`, `completion_only`) produce no work.
`incomplete_limit`, an incomplete snapshot, an unknown schema, a selected unit
whose digest/readiness/gates/requirement references disagree with the change, or
an already archive-ready selection fail admission. The `kiln-cairn-intake`
binary admits the resulting units into the durable ledger; repeating a snapshot
at the same sequence is a duplicate, while a higher sequence with a changed
digest supersedes the previous work. Its `--policy` file is a
`CairnImplementationPolicy`:

```json
{
  "repository": "onix/cairn",
  "project": "cairn",
  "actor": "operator",
  "requested_outcome": "implement",
  "acceptance": [{"id": "accepted", "statement": "reviewed acceptance verified"}],
  "permitted_paths": ["crates/cairn-cli"],
  "permitted_effects": ["write_source"],
  "verification_profile": "<64 lowercase hex>",
  "budget": {"attempts": 2, "minutes_per_attempt": 60},
  "terminal_policy": "verified_pull_request",
  "priority": 1
}
```

## What is not implemented yet

The adapters that produce these records are separate work:

- The GitHub polling schedule and webhook service wiring. Normalization
  and admission are implemented; the service does not yet receive, persist or
  poll them.
- Cairn intake. It must consume Cairn `OnixResearch/cairn#1`'s versioned
  implementation-ready snapshot; that producer is unimplemented. This contract
  intentionally does not fork Cairn lifecycle or readiness semantics.
- The Mneme OpenSpec consumer qualification.
- Claims, execution, candidate acceptance, and publication (issues #45–#48).

The core admission modules remain pure, tested boundaries with no dispatch
authority.

## Attempt metrics

`AttemptMetrics` is an opt-in, default-off receipt for one claimed attempt. It
binds the attempt/fence/work identity and revision, claim and terminal times,
elapsed milliseconds, an `open` flag, optional trace references, and token or
auxiliary-call observations. Missing usage is serialized as an explicit
`absent` observation with a reason, never as zero.

Observation never records or gates anything. `kiln-implementer metrics
--attempt ID [--usage-file FILE] [--record true]` previews the receipt; only
explicit recording retains the first receipt on the attempt. Metrics are not an
admission, verification, publication or incident-scoring condition.
