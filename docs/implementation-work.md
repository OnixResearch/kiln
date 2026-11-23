# Implementation work admission

Kiln owns versioned intake, durable claims/fences, bounded dispatch, independent
candidate acceptance and recoverable publication. Lattice owns one execution;
provider grants and model-service admissions remain separate. The actual shell
consumers and current qualification boundaries are in
[the implementation runbook](implementation-runbook.md).

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

## Implementation authority

Every protected implementation path requires current authority for one exact
request. The closed coverage map `kiln.implementation-authority-coverage.v2` in
`kiln_core::implementation::authority` binds each operation to one Basalt
operation, enforcement boundary, effect class and candidate rule:

| Operation | Boundary | Effect | Candidate |
| --- | --- | --- | --- |
| `ClaimAcquire` | admission | mutation | no |
| `ClaimRenew` | admission | mutation | no |
| `Dispatch` | admission | mutation | no |
| `CandidateSubmit` | campaign | mutation | yes |
| `VerificationAdmit` | verification | mutation | yes |
| `AttemptCancel` | campaign | mutation | no |
| `PublicationPublish` | publication | publication | yes |
| `IntegrationLand` | publication | integration | yes |
| `LifecycleComplete` | lifecycle | lifecycle | yes |

An operation or protected path without an entry denies. No operation implies
another: a claim, renewal or dispatch grant cannot admit submission, acceptance,
publication, integration or lifecycle completion.

The coverage map also classifies every reviewed entry point (each `Implementer`
public function, CLI subcommand, fleet API route and web control route) as
exactly one of:

- `Protected(op)`: admits a fresh exact invocation before the effect;
- `Reconcile(op)`: records or re-observes the outcome of an effect already
  admitted under `op`, bound to that effect's original identity, and never
  starts a new effect;
- `ObservationOnly`: reads state, and confers and consumes no authority;
- `OperatorConfig`: operator-owned configuration, intake or ledger maintenance
  that the deployment authenticates outside implementation authority.

The table must be bounded and free of duplicate entries, and every protected
operation needs a `Protected` entry at its reviewed owner path.

`AuthorityRequest` binds the project, work identity and version, attempt and
fence, the canonical payload digest, the target and the exact candidate
(charter `kiln.implementation.authority.binding`). The adapter adds the trusted
holder subject, deployment audience and authority generation from its operator
configuration, so a caller cannot substitute current deployment state.

The payload digest (`kiln.implementation-authority-payload.v2`) is BLAKE3 over
the operation, the worker incarnation (the attempt's recorded transport launch,
or `pre-launch`), the stable effect identity, the expected remote target state
and the exact payload parts. Every field is length-framed, and the receiving
owner recomputes the digest from its own facts. A fresh invocation of the same
effect reuses the effect identity; only its holder nonce differs. Dispatch
reconciliation therefore presents a new invocation for the original
`DispatchReservation` effect identity rather than authorizing a second dispatch.

### Profiles

The adapter is explicit. Every shell selects a profile through one helper,
`Implementer::select_authority`, from an exported
`kiln.implementation-authority-config.v1` file (`--authority-config FILE` on
`kiln-implementer`, Lattice dispatch and worker, and
`kiln-web-service verify-implementation`, or an `authority_config` path in a
campaign drive profile or fleet configuration) or the legacy
`--local-operator <identity>` shorthand (`local_operator`, or fleet
`authority_operator`, field). Native Git lifecycle/qualification requests and
the campaign native delivery profile accept only `local_operator` so far. Giving both is
refused. Giving neither selects `absent`. The operator authors the file from
`config/implementation-authority.ncl`; Kiln re-checks every Nickel bound when it
reads the export.

- **Absent.** Every protected operation denies.
- **Local operator.** The explicit compatibility profile, also reachable through
  `Implementer::open_with_local_operator`/`select_local_operator`. The decision
  records the operator provenance. It refuses projection-enabled routes and is
  never a fallback for a missing verified credential.
- **Verified UCAN.** Selection builds
  `kiln_adapter_basalt::BasaltAuthority::verified` from the packaged Basalt
  policy or one reviewed policy file pinned by BLAKE3, the deployment
  generation and audience, operator-admitted trusted roots, a static Ed25519 key
  resolver, a bounded proof directory of compact tokens, deny-all caveat
  policies and Kiln's durable replay and revocation stores. Three
  role-separated holder keys, each loaded from its own owner-only seed file,
  sign requests. The coordinator signs claim, renewal, dispatch, candidate
  submission and cancellation; the verifier signs verification admission; the
  publisher signs publication, integration and lifecycle completion. Each
  request is signed afresh with a new nonce over its exact canonical resource,
  and a grant addressed to one holder DID cannot be used by another. Basalt
  verifies the signed holder invocation and policy binding; Kiln owns current
  work, attempt and fence state. Basalt's `durable-authority` feature stays off.

`Implementer::set_authority` remains the seam for adapters and tests that wrap a
composed port. A missing or mismatched profile denies before the mutation,
provider call or credential resolution.

The verified profile's durable state is the SQLite file `<ledger>.authority`
beside the implementer ledger, kept separate because protected admission runs
inside the ledger's `IMMEDIATE` transaction:

- `authority_replay` is keyed on audience, generation, token reference and
  nonce. It is insert-or-deny in its own `IMMEDIATE` transaction as the final
  consuming step, and its rows are never pruned before token expiry.
- `authority_revocation` holds the revoked references of every revocation feed
  (`kiln.implementation-authority-revocations.v1`) imported at selection, one
  union across feeds. `authority_revocation_feed_view` keeps one view per
  feed, keyed by the feed file's canonical path. A process checks only its own
  feed's view, whose age runs from the feed's own `observed_at_ms`, so
  importing never refreshes it and another feed's freshness never counts. An
  absent view, or one observed more than the configured `max_age_ms` ago,
  denies every verified request.
- `authority_decision` keeps the newest 65,536 redacted decision records.

The [runbook](implementation-runbook.md#select-implementation-authority-first)
gives the operator procedure and the GitHub remote-guarantee table.

### Revalidation and decisions

Admission consumes the holder invocation once and retains one live session.
Every protected effect revalidates signatures, proofs, time, revocations, policy
and deployment generation immediately before its durable commit or transport
without consuming replay again. Cancellation, supersession, lease expiry and
fence replacement block later effects even when another valid proof is
presented; unavailable currentness fails closed for new effects. Before Lattice
transport, `admission_before_transport` re-reads the attempt fence inside the
transport transaction, whether or not an admission policy is configured, so a
fence change committed after the authority check refuses transport.

The adapter retains at most 256 redacted decisions in memory.
`Implementer::authority_decisions()` returns that bounded log, and the verified
profile also persists each decision (`kiln.implementation-authority-decision.v1`)
for `Implementer::persisted_authority_decisions`. A decision contains the
principal, project/work/attempt and fence, operation, boundary, effect,
provenance, generation, policy and upstream receipt references, request identity
and denial reason. It contains no capability material, keys, token bytes or
holder signature. A request, decision, receipt or retained session is not
transferable authority and cannot authorize a different effect.

### Non-claims

Authority proves permission for one exact request, not effect completion,
independent acceptance, confinement, resource release or release eligibility. A
successful authorization never creates a passing verification check, discharges
a Cairn obligation or proves provider behavior.

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
- Under an explicitly configured plan-review policy, work in a selected
  repository is also blocked with `plan_not_accepted` until its exact version
  has an accepted plan. Without the policy this reason never appears; see
  [plan review](plan-review.md).
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
revokes its fence and consumes budget. Replacement remains bounded by lineage;
an unacknowledged possible dispatch or explicit unknown provider outcome must be
reconciled before retry. Lease expiry is not provider death or a usage refund.

Before transport, an attempt reserves exactly one dispatch binding: provider
profile, admitted plan identity, deterministic effect identity and idempotency
key. The intake policy MAY supply an exact resolved `base`; dispatch requires it.
A dispatch plan binds the work identity/revision, attempt/fence,
repository/base, isolated workspace, provider profile, admitted paths/effects,
cancellation-only controls, deadline, token/invocation budgets and expected
output contract. Work must admit every path and effect, the deadline must fit
the attempt budget. Ambient credentials are refused; network is available only
for the explicitly bound model-service route and qualified provider namespace.
Repeating the same reservation is idempotent; a different plan or profile
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
Capture and canonical verification resolve that contract through the one
transactional `Implementer::active_work` accessor. Historical lookup requires
the exact current active/unexpired fence plus retained FinishActive lineage;
AbortActive, cancelled, replaced and expired claims cannot read history as
current authority. Publication still refuses superseded results.

The state machine is pure; the durable shell persists transport intent before
calling the separately admitted real worker route. Cancellation, workspace
isolation and observed teardown remain Lattice/provider responsibilities.

### Local terminal-callback regression matrix

The SQLite owner tests in `crates/kiln-implementer/tests/durable.rs` exercise
the actual `record_verification`, cancellation, recovery and claim boundaries:

| Ordering | Retained result | Regression |
|---|---|---|
| Completion, restart, repeated completion, late cancellation | Exact verified receipt and dispatch lineage remain; late cancellation and stale work/fences are refused; no replacement is claimed | `completion_before_cancel_retains_exact_evidence_across_restart` |
| Cancellation, late completion, restart and duplicate callbacks | Cancelled/unknown attempt retains its handoff and dispatch binding without accepted verification; replacement remains blocked until exact termination reconciliation | `cancel_before_late_completion_requires_observed_termination_across_restart` |
| Termination reconciliation, replacement, predecessor callback | Replacement has a higher fence and ordinal two; old verification/authority cannot attach to it or replace the cancelled predecessor | `cancel_before_late_completion_requires_observed_termination_across_restart` |

Repeat with `cargo test -p kiln-implementer --test durable`. These are local
ledger observations, not real-worker termination, resource release or zero
inference-usage evidence; assembled provider qualification remains separate.

## Lattice request dispatch

The existing configured graph route remains an explicit compatibility profile;
it gains no physical-resource/model-service qualification. Strong implementation
execution uses `kiln-lattice-dispatch --worker-request` and the separately adopted
immutable Lattice `implementation-attempt` CLI, not a widened review graph.

`DispatchExecutionBindings` retains exact current `WorkNeedsAttachment`, selected
worker/execution/model/request-admission references and the frozen native request
digest with the attempt/fence. The durable plan survives restart. Readiness is
read-only; actual current owner grants, generation, runtime deadline, work/fence,
source/spec/profile and request bytes are checked before transport.

Transport intent is committed before execution. Re-entry calls native recovery,
never blindly dispatches; explicit recovery refuses a missing prior intent. Raw
events and actual candidate observations are retained separately. Unsupported
namespace/credential/model-service routes fail closed. Source-cohort adoption,
workspace/kernel/egress and real model qualification are explicit operator
prerequisites, not inferred from a configured executable.

## Verified-PR publication

`PublicationPlan` prepares delivery only for a terminal `verified` attempt. It
binds the exact candidate handoff and verification receipt, work/version,
attempt/fence, repository, frozen base, target base branch, candidate/tree, a
deterministic `kiln/impl-<work>-<attempt>` branch, and bounded PR title/body. The work must use
the verified-pull-request or explicit verified-integrated terminal policy. Preparation performs no remote
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
Acceptance moves the attempt to the terminal `verified` state. A verified work
version cannot be claimed again; a new source version remains separately
claimable. Pure receipt admission does not execute checks, create CI facts or publish
anything.

Only `kiln-web-service verify-implementation` executes and admits verification
through the existing canonical Engine. The removed direct implementer verification
path cannot import a caller-supplied success envelope. Frozen programs run in a
separate verifier UID with private source/scratch; Cairn work invokes the existing
native authoritative evaluator over actual observations and exact outer bindings.

## Candidate handoff

`kiln-core` admits one complete candidate handoff for an executing attempt
before any verification. It binds the exact work identity/revision,
attempt/fence, repository and frozen base, candidate commit and tree, sorted
changed paths, the digest of the work's acceptance obligations, the verification
profile, toolchain and environment identities, and retained evidence
references. Every changed path must be inside a work-permitted path, evidence
identities must be unique, and candidate and tree must differ. Repeating the same
handoff is idempotent; a different handoff is refused.

`capture-candidate` independently recomputes actual native snapshot/diff/path
facts and materializes real Git commit/tree objects before retaining a handoff.
The low-level `handoff` command admits only the proposal envelope, never acceptance.
Worker assertions/checklists/exit status are not checks. Honest unchanged native
observations require both explicit already-satisfied policies and independently
executed verification; an empty Produced receipt remains invalid.

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

## Qualification boundaries

The implementation runbook and `docs/implementation-qualification.md` distinguish
source/fixture coverage from real bug/feature/no-change, model, cancellation and
remote-effect evidence. No live qualification is asserted by this document.

Current authenticated-model credential delivery, automatic GitHub landing and
conditional source mutation are unqualified and fail closed. Verified-PR remains
different from exact independently observed integration; `VerifiedIntegrated` is
the explicit landed terminal policy. Source closure and reviewed Cairn lifecycle
observations are separately journaled, exact-version effects; no post-review
source lifecycle mutation or missing-directory completion is inferred.

Work/resource requirements are sealed against the existing canonical admission
policy and exact current work version. `ImplementationObservation` implements a
bounded SQLite read-only port: no schema creation, claim or reservation from GET.
Missing current attachments, stale work/policy and malformed records remain
distinct. A policy/source change cannot reuse historical requirements.

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
