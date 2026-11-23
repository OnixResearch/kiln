# Implementation service runbook

Kiln owns work, current claims/fences, cross-attempt budget, independent acceptance and publication. Lattice executes one bounded attempt. The supported consumer is the fixed trusted dispatcher, the canonical Engine verifier and the separately credentialed publisher—not a worker success manifest. Kiln maintainers own these commands; the operator owns source/profile enrollment, physical/model grants and deployment.

This is an implementation procedure, not a live qualification receipt. No real bug/feature, OMP/model call, GitHub publication, landing or lifecycle mutation is claimed here. The executable campaign and evidence requirements are in [implementation qualification](implementation-qualification.md).

## Select implementation authority first

Claims, renewal, dispatch, candidate submission, verification admission,
cancellation, publication, integration and lifecycle completion deny until a
profile is selected. The request, profile, revalidation and decision rules are in
[implementation authority](implementation-work.md#implementation-authority).

### Select one profile

Every shell that owns a protected command takes at most one of two selectors:

- `--authority-config FILE`: the JSON exported from the operator's
  `config/implementation-authority.ncl`
  (`kiln.implementation-authority-config.v1`);
- `--local-operator OPERATOR`: the legacy shorthand for the `local_operator`
  profile.

Passing both is refused before the ledger opens. Passing neither selects the
`absent` profile, which denies every protected operation. A profile is never
inferred from a role, provider capability or environment variable.

```text
nickel export --format json config/implementation-authority.ncl > authority.json
kiln-implementer <command> --database DB --enrollment ENROLLMENT \
  --authority-config authority.json ...
kiln-lattice-dispatch --authority-config authority.json ...
kiln-lattice-dispatch --worker-request REQUEST ... --authority-config authority.json ...
kiln-web-service verify-implementation --authority-config authority.json ...
```

Campaign drive profiles and fleet configuration carry an optional
`authority_config` path, exclusive with their `local_operator` (fleet:
`authority_operator`) field. A campaign verifier role's argument list must carry
the same selector as the profile. **Limitation:** native Git
lifecycle/qualification requests and the campaign native delivery profile still
accept only `local_operator`; they cannot select the verified profile yet.
Every shell, these included, composes through one helper,
`Implementer::select_authority`, which re-checks every bound of the Nickel
contract (closed records, at most 16 trusted roots and 64 static keys, names of
at most 256 bytes, absolute paths of at most 4,096 bytes without `..`).

The three profiles are:

- **`absent`**: denies every claim, renewal, dispatch, candidate submission,
  verification admission, cancellation, publication, integration and
  lifecycle completion.
- **`local_operator`**: the explicit compatibility profile. The decision records
  the operator provenance; it never admits a projection-enabled route and is
  never a fallback for a missing verified credential.
- **`verified_ucan`**: the composition described below.

### Verified UCAN composition

At selection Kiln reads, once and within fixed size bounds:

- the Basalt policy: the packaged policy, or one reviewed policy file whose
  BLAKE3 must equal the configured `blake3`;
- the deployment `audience` and non-zero `generation`, and the admitted
  `trusted_roots`;
- a static Ed25519 key resolver built from `keys` (`did:key` to public key);
- the proof store: at most 256 compact `*.ucan` tokens from `proofs_dir`;
- the revocation feed (below), imported into the durable view;
- the three holder seed files.

Unknown caveats deny. Basalt's `durable-authority` feature stays off; Kiln owns
the replay, revocation and decision stores.

**Role-separated holders.** `holders.coordinator`, `holders.verifier` and
`holders.publisher` each name a distinct owner-only (`0600`) file holding one raw
32-byte Ed25519 seed. Selection refuses a shared path, a group- or
world-readable file, a wrong length, or a holder DID without exactly one current
grant in the proof directory. The seed never enters argv, logs or a request.
Roles follow `GrantRole::of`. The coordinator (coding role) signs
`claim-acquire`, `claim-renew`, `dispatch`, `candidate-submit` and
`attempt-cancel`. The verifier signs `verification-admit`. The publisher signs
`publication-publish`, `integration-land` and `lifecycle-complete`. Each request is signed
afresh with a new nonce over its exact canonical resource. A grant addressed to
one holder DID cannot be used by another holder, so a compromised coordinator
key cannot sign verification or publication requests.

**Durable state.** The verified profile keeps its state in the SQLite file
`<ledger>.authority` beside the implementer ledger. It is a separate file because
protected admission runs inside the ledger's `IMMEDIATE` transaction; a second
connection to the ledger file would wait on that writer lock. A busy authority
database denies after a bounded wait. Its tables are:

| Table | Contents and rule |
| --- | --- |
| `authority_replay` | `(audience, generation, token_ref, nonce, resource_digest, expires_ms)`, keyed on the first four. Insert-or-deny in its own `IMMEDIATE` transaction as the final consuming step before the effect. A duplicate denies. `expires_ms` is the later of the token expiry and admission time plus `replay.retention_ms`; rows are pruned only after it. |
| `authority_revocation` | `(proof_ref, revoked_at_ms, revocation_generation)` for every revoked proof reference. |
| `authority_revocation_view` | One row holding the imported revocation generation and its `observed_at_ms`. |
| `authority_decision` | `(seq, record)`, the redacted `kiln.implementation-authority-decision.v1` JSON. The newest 65,536 are kept; `Implementer::persisted_authority_decisions` reads at most 1,024 per call. |

**Revocation feed.** `revocation.path` names a
`kiln.implementation-authority-revocations.v1` document
(`{schema, generation, revoked: [Hex32]}`, at most 65,536 references). Selection
imports it; a feed whose generation is older than the recorded view is refused.
The view fails closed: if it is absent, or was observed more than
`revocation.max_age_ms` ago, every verified request denies. Each selection
imports the feed anew; refresh it, and restart long-running shells, before the
view goes stale.

**Payload v2.** Every request carries `request_digest` = BLAKE3 over the domain
`kiln.implementation-authority-payload.v2`, the operation, the worker
incarnation (the recorded `TransportLaunch` identity, or `pre-launch`), the
stable effect identity, the expected remote target state and the exact payload
parts. Every field is length-framed. The receiving owner recomputes the digest
from its own facts, so a caller cannot substitute a payload.

**Reconciliation.** `reconcile_dispatch` presents a fresh invocation, with a new
nonce, for the original `DispatchReservation` effect identity. It settles that
effect; it never authorizes a second dispatch. A recovered publication
reservation grants only read-only reconciliation.

**Fence at transport.** `admission_before_transport` authorizes and
revalidates, then re-reads the attempt fence inside the transport transaction.
A fence change committed after the authority check refuses transport, whether
or not an admission policy is configured.

The adapter records bounded redacted decisions and revalidates immediately
before every protected commit or transport. Missing authority, cancelled or
superseded attempts, lease expiry and unavailable currentness deny before the
effect. Reconciliation and cleanup still require their own authorization.

### Remote guarantees (GitHub)

Kiln's authority decides whether Kiln may attempt one exact effect. It does not
make GitHub's state change conditional. The table lists what GitHub itself
guards for each remote effect Kiln performs or observes, and what Kiln does
locally. A local precheck and read-back narrow a race window. They are never a
substitute for a remote atomic guard.

| Effect (Kiln route) | What GitHub makes conditional | What GitHub does not make conditional | Kiln's local behaviour |
| --- | --- | --- | --- |
| Candidate branch push: Git smart-HTTP `receive-pack` (`kiln-implementer publish`, branch effect) | Each ref-update command carries an old object ID, and receive-pack updates the ref only if it still equals that old ID[^pack]. That makes a push a per-ref compare-and-swap. Kiln sends the zero old ID (create-only) with a non-force refspec. | The push is not bound to Kiln's attempt or fence. GitHub runs its own Git server, and **[not verified]** that it applies the same per-ref check under concurrency. A lost response leaves the outcome unknown. | Before pushing, Kiln reads the remote ref. If it already equals the candidate, Kiln reuses it; any other head is a conflict and is never overwritten. A durable effect reservation is committed before transport. libgit2's push-negotiation callback refuses an advertised old/new pair that differs from the guarded one. After a rejected or unknown push Kiln reads the ref back; if it is still absent, the outcome stays unknown and needs reconciliation before retry. A recovered reservation never pushes again. |
| Pull request creation: REST `POST /repos/{owner}/{repo}/pulls`[^create] | Nothing about prior state. GitHub validates that the head and base exist. | No idempotency key and no `If-Match`: GitHub does not support conditional requests for `POST`/`PUT`/`PATCH`/`DELETE` unless an endpoint says otherwise[^cond]. **[not verified]** GitHub's exact refusal of a duplicate open pull request. | Before creating, Kiln lists `pulls?state=all&head=owner:branch&base=…` and reuses the one exact match. More than one match or a paginated result is unknown. Kiln re-reads the branch (read-only) before the request. A `409`/`422` triggers one re-read, and an unreadable response is unknown. A recovered reservation only reads (`ReconcileOnly`). |
| Pull request merge/landing: REST `PUT /repos/{owner}/{repo}/pulls/{n}/merge`[^merge] (**refused today**) | The optional `sha` must equal the pull request **head**, or the merge fails with `409`. | The **base** branch head is not guarded. The target can move between observation and merge, and no REST write accepts `If-Match`[^cond]. | `kiln-implementer land` and `GithubLandingClient::land` always refuse ("unattended landing is unsupported"); the pull request stays ready for review. `recover-integration` only reads a pull request that was merged externally. It requires the exact head and base ref, the merge commit's parents and tree mapping, check-run evidence, and a base head that descends from the merge commit (`compare`). |
| Source issue close: REST `PATCH /repos/{owner}/{repo}/issues/{n}`[^issue] (**observation only**) | Nothing. | The PATCH is unconditional: no source-version guard and no `If-Match`[^cond]. | Kiln never issues the PATCH. `close-source` reads the issue and requires `updated_at` to equal the plan's expected revision. A closed issue yields a receipt. An open issue is refused and left for operator reconciliation. |
| Commit status and check observation: `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`[^checks] | Only cache validation: `ETag`/`If-None-Match` on `GET` returns `304` when nothing changed[^cond]. That is not a consistency guarantee. | A read is a point-in-time snapshot, and checks can be re-run or added afterwards. | Landing observation reads the check runs for the exact head as evidence, never as permission, and refuses a result that spans more than one page. The separate CI status adapter's `POST …/statuses/{sha}`[^status] is an unconditional, append-only write outside implementation authority. |
| Lost-response reconciliation reads: ref advertisement, `GET` pull list, pull, commit, compare and issue | Nothing. These are snapshots. | A read cannot tell whether a lost write applied or a concurrent writer acted. | The durable reservation turns any retry into `ReconcileOnly`. Kiln adopts only a result that matches the exact candidate, branch, base and plan. A mismatch is a conflict, and absence stays unknown rather than triggering a blind second mutation. |

These remote conditional and lost-response guarantees are **LIVE-only
qualification** (#47/#48). Local bare-Git and loopback-HTTP fixtures exercise
Kiln's prechecks, reservations and read-backs. They do not prove GitHub's
server behaviour, and nothing here is proven locally against GitHub.

[^pack]: Git pack protocol, reference update request:
    <https://git-scm.com/docs/gitprotocol-pack> ("validate each reference that is
    being updated that it hasn't changed while the request was being processed
    (the obj-id is still the same as the old-id)").
[^create]: <https://docs.github.com/en/rest/pulls/pulls#create-a-pull-request>
[^merge]: <https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request>
    (`sha`: "SHA that pull request head must match to allow merge"; `409` "if
    sha was provided and pull request head did not match").
[^issue]: <https://docs.github.com/en/rest/issues/issues#update-an-issue>
[^cond]: <https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api#use-conditional-requests>
    ("Conditional requests for unsafe methods, such as `POST`, `PUT`, `PATCH`,
    and `DELETE` are not supported unless otherwise noted").
[^checks]: <https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference>
[^status]: <https://docs.github.com/en/rest/commits/statuses#create-a-commit-status>

## Intake, policy and immutable preparation

Admit authenticated GitHub issue deliveries using `kiln-implementer drain`/`run` or `poll`/`poll-run`, or use `kiln-cairn-intake` for an actual complete implementation-readiness snapshot. Policies supply obligations, effects, source aliases, exact Git base, attempt budget and terminal policy; issue prose does not grant effects. Duplicate/stale intake remains idempotent. New source versions atomically apply supersession; `admit_with_supersession` exposes explicit abort/finish policy without rewriting the frozen contract.

Before starting intake, preflight the exact policy and enrollment files with `kiln-implementer policy-check --policy FILE --enrollment FILE`. It reads them with the types and reader `poll` uses, applies the policy validation every intake pass applies, and admits one synthetic open issue from an enrolled actor through poll's work construction and ledger admission in an in-memory ledger. It needs no database, credential or network, prints `{"policy":"admitted","repository":"owner/repo"}` on success, and exits 2 with the refusal otherwise. A deployment's policy-check stage should call this command rather than re-checking JSON keys itself, so the preflight cannot admit a policy `poll` rejects. It does not prove that any issue is selected or that the GitHub credential and transport work.

Implementer JSON input errors name the file and say whether it "is not valid JSON" (a syntax error) or "does not match its schema" (for example ``missing field `contract_version` ``), always with the line and column. Messages never quote input: an unknown member name is reported as "unknown field (name withheld)" with the type's expected field names, and a rejected value only by the kind of mismatch and the expected type.

Default operator intake selects `AbortActive`. When an embedder explicitly selects
`FinishActive`, capture and canonical verification use the single transactional
`Implementer::active_work` accessor: a historical frozen contract is returned only
for the exact still-current, active, unexpired attempt/fence whose retained
`superseded_by` matches the newer current revision. Cancelled, aborted, replaced,
expired or unproved historical attempts fail closed. Superseded candidates never
gain publication authority; the newer source remains separately admitted work.
This is the supported source/fixture path, not a claim of an exercised live pilot.

Human-authored policy remains typed Nickel (`config/implementer-policy.ncl`, `config/implementation-worker.ncl`). Export operator-reviewed inputs before passing JSON to bounded CLI consumers.

<!-- r[impl kiln.implementation.intake_selection] -->
A GitHub intake policy may carry an explicit `selection` of issue numbers and/or exact label names (`selection = { issues = [1] }` in the Nickel input). Webhook drain and polling then admit an open issue as work only when its number is listed or it carries one of the labels; every other open issue is observed as `unselected` (`PollSummary.unselected`) and never admitted. Closed issues are still observed as closed. Without `selection` every enrolled open issue remains admissible under the repository-wide policy. An empty selection, issue `0`, or a malformed or unbounded label fails before any provider call. Selection never retracts work that was already admitted.

Prepare the verifier profile through its native codec:

```console
kiln-implementer prepare-verification-profile \
  --profile /etc/kiln/verification-seed.json \
  --output /var/lib/kiln/verification-profile.json
```

The output reports the canonical profile identity and `checks_executed:false`. Use that identity in intake policy. Preparation computes frozen toolchain/environment identities; it does not run checks or establish acceptance.

Configure opt-in bounded fair admission with `kiln-implementer admission --command-file FILE`; `admission-status` reads its retained ledger. Commands configure policy/accounts, enqueue, select/claim, retain owner reservations/replies and settle observed usage. `--admission-observations FILE` supplies fresh provider facts to actual dispatch. Selection, inventory eligibility, a configured worker and a model deployment are never grants. Unknown usage remains conservatively charged.

For one explicitly registered host-local owner, the reviewed `config/fair-admission-host-owner.ncl` export drives the native records instead of hand-written commands: `host-owner-admit` (policy, account and queue), `host-owner-observe` (fresh eligibility and usage), `host-owner-reserve --attempt ID --fence N` (the owner-committed reservation dispatch checks) and `host-owner-release --attempt ID --fence N` (settlement only after the attempt is observed terminal). See [the fair-admission guide](fair-admission.md#registered-host-local-owner).

Under an enabled admission policy, `claim` selects and binds through admission's claim command so the queue/account and current attempt are associated; it needs `--local-operator`, which is an explicit opt-in guard naming the local operator, not authentication. Without a policy, `claim` remains the existing configured-route compatibility consumer. `inspect-work --work ID` returns exact retained work, source versions and native acceptance digest.

## One real bounded worker attempt

Prepare an isolated workspace from the exact admitted Git base. No linked `.git` pointer, publisher credentials, verifier-private material or database path is exposed to the coding child. The selected runtime uses the already implemented bubblewrap OMP route. It must have a reviewed OMP build at revision `e325ffc73ab86e03bdd20a91b77d8e2b3964c906`, exact executable BLAKE3 and the selected kernel/provider qualification.

```console
lattice implementation-attempt prepare \
  --request /var/lib/kiln/worker-seed.json \
  --output /var/lib/kiln/worker-request.json
```

The native producer reads the actual baseline snapshot, computes its own snapshot/profile/envelope/request identities and writes a new frozen request. Its strict schema is `lattice.implementation-attempt-run.v3`; the implementation is `src/implementation_attempt/cli.rs` in the explicitly adopted Lattice source cohort (`db64cd65`, `docs/candidate-worker.md`). Seed envelope work/version/attempt/fence, source native ID/version, acceptance digest, exact repository/base, verification profile and policy digest come from actual Kiln observations. A v3 seed also declares `worker_compute` (`{"kind":"host_local"}` for the standalone route), `execution_binding` (`lattice.execution-binding.v1` with the same compute and exactly one model route named `primary`), the `omp.model_broker` route and `network:false`. Preparation is not admission or execution.

The request must also carry `dispatch_plan`, the identity of the exact plan Kiln reserves. That identity frames the plan's native request profile, which only preparation computes, so prepare twice: prepare the seed once to read `envelope.worker_profile`, set it as the plan's `native_request_profile`, compute the plan identity, add it to the seed as `dispatch_plan`, and prepare that seed again. The profile revision does not frame `dispatch_plan`, so the second preparation keeps the same profile. The campaign does this automatically (`campaign-ATTEMPT.profile.json` and `campaign-ATTEMPT.planned-seed.json`); `kiln-implementation-fixture bind-execution` emits the planned seed as `worker_seed` for the local fixture. A request without the reserved `dispatch_plan` is refused before transport intent.

`prepare-needs --work ID --needs-file FILE --output FILE` seals explicit profile/resource/model requirements against the current canonical admission policy; `attach-needs --needs-file FILE` retains them. `needs --work ID --policy-identity ID` refuses moved policy/work. `DispatchExecutionBindings` includes that exact attachment, the selected worker/execution/model/request-admission references and the native request-byte digest. The references preserve provider identities; the actual owner-grant gate must still pass. A changed profile/model/request requires a new admitted binding, not an ambient endpoint substitution.

Reserve the bounded plan, then execute through the actual coordinator consumer:

```console
kiln-implementer reserve-plan --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json --plan-file /var/lib/kiln/dispatch-plan.json

kiln-lattice-dispatch --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --dispatch-plan /var/lib/kiln/dispatch-plan.json \
  --worker-request /var/lib/kiln/worker-request.json \
  --worker-transport-profile /etc/kiln/worker-transport.json \
  --admission-observations /var/lib/kiln/current-provider-grants.json
```

Transport schema `kiln.lattice-implementation-cli.v2` binds immutable `lattice_program`, `executable_blake3`, `state_directory` and private `output_directory`. `config/implementation-worker.ncl` and campaign binding admit only v2; the direct `kiln-lattice-dispatch --worker-request` runtime keeps its pre-existing v1 branch for plans without a native request profile. It runs only the selected immutable executable. Current work/fence, frozen request bytes, the reserved `dispatch_plan`, source/spec/profile/workspace bindings, queued runtime deadline and fresh owner grant generations are checked before transport. Selected plans/bindings and dispatch intent are durable. A second invocation automatically calls native recovery; explicit `--recover true` refuses absence of a prior transport intent.

The transport intent of an attempt's first `run` records its launch: the exact `lattice_program`, its BLAKE3 and `state_directory` (`TransportLaunch`), in the same SQLite transaction that persists the intent (`begin_dispatch_transport`). The launch is stored under its own implementation-state kind, `transport_launches`, not in the `attempts` document, so an older Kiln binary that reads and re-saves `attempts` during a rolling upgrade cannot erase it. A recorded launch is never overwritten. When an intent already exists, `begin_dispatch_transport` returns that recorded launch, and `recover` uses only it, never the current profile's, so neither a changed profile nor a profile or process whose owner-grant gate was refused (and that therefore never reached the intent) can move recovery to another root. If the intent has no recorded launch (it was persisted before launches were recorded), the recorded root is no longer a real directory (a symbolic link counts as missing) or the recorded program's BLAKE3 differs, Kiln does not invoke Lattice and keeps the unknown outcome for reconciliation. Settling an attempt removes its recorded launch in the same transaction. A record is consulted only while its intent exists. The recorded root is assumed durable: losing or replacing it in place voids its no-start evidence.

On a first launch the steps run in this order: claim authorization and the credential environment; plan reservation and request seal; queue/lease/budget checks; every fallible filesystem step (private output directory, frozen request and state root); the owner-grant gate (`admission_before_transport`, which moves committed owner reservations to unknown and the queue item to dispatch intent); the transport intent with its launch (`begin_dispatch_transport`); then Lattice. An unwritable state root or a conflicting frozen request therefore fails with the owner grants still committed, and a retry after fixing the cause proceeds. **Open item (predates this change):** a crash between the owner-grant gate and the transport intent (`crates/kiln-adapter-lattice/src/worker.rs`, `admission_before_transport` then `begin_dispatch_transport`) leaves unknown owner grants without a transport intent, so neither a fresh run (the gate refuses non-committed grants, `crates/kiln-implementer/src/admission.rs`) nor recovery can proceed until an owner reconciles the grants. The two transactions are not merged yet.

Between the queue/lease/budget checks and the filesystem steps, a first launch derives Lattice's `lattice.kiln-budget-lineage.v1` from the admission ledger. When the attempt holds a committed owner reservation, Kiln appends it as the request's `lineage` member. The sent bytes are frozen as `request-<BLAKE3 of the sent bytes>.json` in `output_directory`, and the transport intent records that file's path and BLAKE3 with the launch. Lattice binds the attempt to those bytes. Run any later `implementation-attempt recover`, `cancel`, `inspect` or `export-candidate` against the recorded file, never against `worker-request.json`. A retry refused before any transport start writes a new file for its own bytes, so a changed lineage never blocks it. An inconsistent lineage is refused with a typed `LineageRefusal` code before the owner-grant gate, and nothing is started. `--recover` reads exactly the recorded file, wherever the current profile's `output_directory` points. When that file is missing or crossed it refuses with "Lattice was not invoked for recovery; the first launch may have run; retain the unknown outcome and reconcile". The `--dispatch-plan` workflow-exchange route checks the same lineage, but its `lattice.workflow-request` has no field to carry it. See [fair admission](fair-admission.md#budget-lineage-sent-to-lattice).

Lattice's typed `lattice.implementation-attempt-refusal.v1` records are settled by their `dispatched` field only for the attempt's first `run`, which is launched only when no transport intent exists. A `run` refusal of this attempt (same work, attempt and fence) that is exactly Lattice's record (no unknown or duplicate fields; `revision_refusal` only for the agent-revision reasons) with `dispatched:false` and reason `crossed_attempt`, `profile_revision_superseded`, `revision_not_admitted` or `revision_blocked` ends the attempt as an infrastructure failure with cause `lattice_refused_before_effect:REASON`; nothing remains to reconcile and the command still exits non-zero. `stale_fence` is kept for reconciliation: Lattice `db64cd65` checks the fence before it writes the execution intent, so a concurrent invocation can still start after another was refused as stale. Every refusal returned by `recover`, every `dispatched:true` refusal (`substituted_request`, `export_revoked`, a superseded profile with a retained intent), an unknown reason, and a crossed, contradictory or malformed record (for example one also carrying a result or artifacts) keeps the attempt for reconciliation. A bound `recover` that reports exactly Lattice's `not_started_closed_by_recovery` record for the same envelope, request identity and plan ends the attempt as `lattice_closed_before_execution` and records no workflow execution.

Use the actual trusted dispatcher host UID (for example 931) in verifier authority. A unit-only ledger group belongs to the fixed trusted dispatcher, not the sandboxed OMP program: the coding child sees only its explicit private-root mounts. Do not pretend a child changed host UID or grant it publisher credentials. Separate verifier UID (for example 933) and private acquisition/scratch roots are required.

The OMP worker never receives a network device. Its only route is the attempt-local Lattice model broker, which relays bounded chat-completion requests for the one admitted model to the broker endpoint; a request with `network:true` or the removed `network_namespace` field is refused by Lattice before any effect. When `omp.model_broker.credential` names `{"provider":"env","key":"NAME"}`, the dispatcher passes exactly `NAME` from its own environment to the `run` command, which resolves it on the host broker; the worker never sees it. Every other variable is withheld, and `recover` and `cancel` receive none. A missing or empty variable, a non-`env` provider, a `field`, a malformed name or a reserved process-control name (`PATH`, `HOME`, `LD_*`, `LATTICE_*` and similar) is refused right after the current claim is authorized, before plan reservation, request sealing, owner-grant admission and the transport intent. Without a declared credential the environment stays fully cleared.

Native output retains `artifacts.raw_events`, `artifacts.omp_observation`, envelope and actual candidate receipt. Cancellation via SIGINT/SIGTERM is forwarded by the coordinator to `lattice implementation-attempt cancel --request FILE --state DIR`; Lattice persists an exact request and invokes its existing process-group cancellation. Requested cancellation is not observed termination. Unknown teardown/controller loss quarantines the workspace and preserves debit; no automatic release, refund or successor reuse follows.

`kiln-implementer recover` reports expiry and pending reconciliation, always with `project_complete:false`. `reconcile-termination` accepts an explicit operator/provider termination-reference observation; a timeout or missing process is not such evidence. Expired/cancelled/replaced fences cannot accept later candidates.

### Worker completion and cancellation races

<!-- r[related kiln.implementation.execution_identity] -->
Every worker callback carries the attempt identity and fence and is checked against the current ledger. A provider run identity binds at most one attempt: acknowledging or reconciling a run that another attempt already records is refused with `implementation_dispatch_crossed`, so a predecessor's run cannot become its replacement's execution. Deterministic fixtures cover the #45 race matrix; `crates/kiln-core/src/implementation/attempt.rs` holds the pure ledger cases and `crates/kiln-implementer/tests/durable.rs` the store across restart.

| Case | Fixture |
| --- | --- |
| Cancellation, then a late candidate, acknowledgement, exit or repeated cancellation | `late_callbacks_after_cancellation_never_attach_or_resurrect`; `cancel_before_late_completion_requires_observed_termination_across_restart` |
| Completion, then a late cancellation | `completion_before_cancel_retains_exact_evidence_across_restart` |
| Repeated callbacks with the ledger restored between each pair | Both cancellation fixtures above |
| Predecessor callbacks after a replacement starts, including its run label | `predecessor_callbacks_never_settle_or_feed_the_replacement`; `abort_active_replaced_and_expired_fences_never_resolve_history`; `cancelled_and_superseded_attempts_lose_unexpired_authority_atomically` |
| A callback for an attempt that was never admitted | `callbacks_for_an_unadmitted_attempt_create_nothing` |
| A lost acknowledgement recovers the original effect and attempt | `ambiguous_dispatch_reconciles_one_execution_without_redispatch`; `dispatch_reconciliation_survives_restart_without_duplicate_execution`; `possible_dispatch_requires_recovery_and_unknown_retry_is_fenced` |
| Cancellation request, acknowledgement and observed termination stay distinct | `late_callbacks_after_cancellation_never_attach_or_resurrect`; `possible_dispatch_requires_recovery_and_unknown_retry_is_fenced` |
| The same races through an assembled provider | Not run: needs the Lattice implementation route (Lattice #6/#7) and deployment qualification (#49) |

None of these fixtures is a provider observation. Refused callbacks create no attempt and spend no attempt budget; an unknown outcome stays unknown until `reconcile-termination` records an observed termination.

## Host-derived Git acquisition and canonical verification

The worker's file-map snapshot is not a Git commit. The trusted acquisition consumer independently recomputes native base/candidate/diff identities, complete changed paths, file modes and allowed edits, then writes a real Git object candidate in the trusted acquisition repository:

```console
kiln-implementer capture-candidate --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json --attempt ATTEMPT --fence FENCE \
  --dispatch-plan /var/lib/kiln/dispatch-plan.json \
  --profile /var/lib/kiln/verification-profile.json \
  --authority /etc/kiln/verifier-authority.json \
  --candidate-root /var/lib/kiln/candidate-export \
  --host-receipt /var/lib/kiln/host-receipt.json \
  --handoff-output /var/lib/kiln/candidate-handoff.json
```

The host receipt is actual native output, never a hand-authored passing manifest. Source exports include additions/deletions/untracked source bytes; symlinks, hardlinks, special files, unsupported modes and crossed bindings fail closed. Acquisition does not mutate protected target refs.

Only the existing canonical Engine can complete verification:

```console
kiln-web-service verify-implementation --profile /etc/kiln/ci-service.json \
  --database /var/lib/kiln/work.sqlite --enrollment /etc/kiln/enrollment.json \
  --attempt ATTEMPT --fence FENCE \
  --verification-profile /var/lib/kiln/verification-profile.json \
  --authority /etc/kiln/verifier-authority.json
```

This executes the frozen immutable verification profile in verifier-owned snapshots, retains actual check/artifact observations, invokes Cairn's existing authoritative completion evaluator when required and admits results through canonical CI. There is no `kiln-implementer verify` or JSON success-receipt import. Existing Engine single-writer/store authority failures remain explicit; this command does not stop/restart a running service. Failed/unknown execution is retained, not converted into a passing receipt. The existing integration queue owns fresh candidate CI on target movement.

For bug profiles, the same admitted reproducer must fail baseline and pass candidate. Feature profiles name explicit acceptance cases. An unchanged OMP result remains generic failure unless `allow_unchanged` was explicitly admitted: native `lattice.unchanged-candidate-observation.v1` is a host-derived proposal, not empty `produced`. Both `VerifiedOrAlreadySatisfied` and verifier `allow_already_satisfied` are required; acquisition fixes Git candidate exactly to baseline and independent checks still run. No empty PR/landing is attempted for this terminal outcome.

Cairn acceptance/spec/registry/policy cannot be weakened. Source-changing lifecycle preparation must occur before review, using an explicitly frozen `prepared_lifecycle_tree` where applicable. The checker executes no lifecycle mutation and missing/archived directories do not prove completion.

## Opt-in attempt outcomes and effort

<!-- r[related kiln.implementation.outcome_metrics] -->

Kiln maintainers own this report over the existing attempt ledger. It is an
observation, not a check, admission rule, budget settlement or publication gate.
Worker terminal state and the controller's verification outcome are retained
separately on the exact attempt/fence. `Completed` does not mean verified.
The normal verifier and attempt state transitions remain authoritative.

```console
kiln-implementer metrics-report \
  --database /var/lib/kiln/work.sqlite --enrollment /etc/kiln/enrollment.json \
  --from-ms "$FROM_MS" --to-ms "$TO_MS"
```

The JSON receipt schema is `kiln.implementation-attempt-report.v1`. Bounds are
Unix epoch milliseconds selecting claims in `[FROM_MS, TO_MS)`, not completion
times or an as-of snapshot. A selected attempt may finish after `TO_MS`.
`totals` and lexicographically ordered `groups` use the same aggregation. Group
keys retain exact worker/provider profiles and declared harness/model labels
where known; null means unknown. A selected model label is not attestation of
served weights or proof that a model request succeeded. No profile ranking or
statistical-significance claim is produced; do not interpret small gaps without
considering sample sizes and uncertainty.

Only rows with a retained end timestamp enter outcome and effort denominators.
`attempts`, `finished`, `open` and `terminal_without_end` expose that coverage.
`verified_passes` requires the existing Verified phase and accepted independent
verification receipt, never metrics alone. `nonpasses` includes every other
finished disposition, including blocked, cancelled and unknown. An in-progress
verifier rejection is retained but enters the report's failure cohort only when
the existing controller closes the attempt. Missing historical observations are
not reconstructed from worker narratives.

Every ratio is either `{numerator, denominator}` or null when unavailable. Raw
counts preserve the sample sizes; multiply a share by 100 for a percentage.

| Field | Numerator | Denominator |
| --- | --- | --- |
| `rejected_completed_of_nonpasses` | Completed workers rejected by the controller | All finished nonpasses |
| `rejected_completed_of_completed` | Completed workers rejected by the controller | All finished Completed workers |
| `turns.per_verified_pass` | Model requests across all finished attempts | Verified passes |
| `turns.verified_pass_mean` | Model requests on verified passes | Verified passes |
| `turns.failed_share` | Model requests on finished nonpasses | Model requests across all finished attempts |
| `elapsed_ms.per_verified_pass` | Total claim-to-end wall milliseconds, including failures | Verified passes |
| `elapsed_ms.verified_pass_mean` | Claim-to-end wall milliseconds on verified passes | Verified passes |

Failure categories are disjoint, in this order: typed deadline or exact ledger
lease expiry, Completed-but-rejected, worker Failed, Aborted/cancelled, then
other/unknown. Raw Completed-and-rejected counts remain visible independently
of category precedence. Unknown/incomplete verification is not rejection.

For **subscription credentials, cost means model turns (model requests), tokens,
quota-window share when available, and elapsed wall time—not dollars**.
Wall time includes claim/queue/verification time and is not model-service time;
overlapping attempts' elapsed values are summed, not treated as unique clock time.

The current `lattice.omp-jsonl-observation.v1` cohort reports terminal state but
does not measure model requests. Its turn observation is
`{"status":"absent","reason":"not_instrumented"}`. `event_count`, Agent-node
count, configured turn limits and reserved budget are never substituted for
usage. Lattice #13 owns qualification of measured usage; unsupported cohorts
remain absent until an exact supported observation contract is adopted.
The trusted consumer reads only the bounded native artifact bound to this
attempt, retains no session body in metrics, and explains unavailable or
malformed observations without changing native candidate admission.

`metrics` still previews by default; only `--record true` retains its first
receipt. Without a supplied usage value, it uses retained worker observations
where available. An operator may supply supported observations with
`--usage-file /absolute/usage.json`, for example:

```json
{"model_turns":{"status":"observed","turns":12}}
```

Use `{"status":"incomplete","turns":5,"reason":"provider_unavailable"}`
for a known lower bound with unknown remainder, or explicit `absent` with a
reason when no count is available. Explicit observed zero is distinct from
missing data. A retained explicit count/lower bound takes precedence over the
worker fallback; reporting never overwrites the original receipt.

`turns` and `elapsed_ms` expose complete, absent and incomplete sample counts
for all attempts, verified passes and nonpasses. Incomplete lower bounds remain
separate from complete totals. Full-window per-pass/share ratios are null if
any contributing sample lacks complete coverage. Verified-pass means require
complete pass coverage only. Separately labeled `complete_cohort_*` ratios use
only complete samples and their own pass denominators; they are not estimates
of the full population. No known pass means no per-pass ratio, and no observed
turns means no failed-turn share.

The current native response has no typed deadline when its OMP terminal stream
cannot be normalized. That terminal remains unobserved; blocker prose is never
parsed into a deadline. Deadline totals require a typed provider/controller
observation or the ledger's exact lease-expiry disposition.

## Publication, integration and source reconciliation

`prepare-publication --publication-file FILE` freezes the exact verified candidate, work/version/fence, branch and concise source/change/validation/limitations description. `publish --publication-file FILE --secret-file FILE --remote-profile FILE` reserves branch and PR effects separately before transport, pushes captured candidate objects through the guarded native Git transport, verifies the remote ref, then creates/reconciles the PR through GitHub's API. Lost responses grant reconciliation-only authority; unexpected heads, all-state PR ambiguity and incomplete pagination refuse progress.

The deny-unknown-fields `kiln.github-publication-source.v1` profile is operator-owned and binds `repository_path`, a `NativeRemoteProfile`, its `remote_identity`, and explicit `fixture` context. The source repository is resolved to the exact `<attempt>-<fence>/candidate` export before any push. Production requires the exact enrolled `https://github.com/<owner>/<repo>.git` URL; its scoped Git credential is never offered to another URL. `local_bare_fixture` is accepted only with `fixture: true` and the feature-gated loopback GitHub fixture endpoint. No GitHub REST branch-ref creation substitutes for object transfer.

<!-- r[related kiln.github.landing.merge.scenario.recovered] r[related kiln.github.landing.merge.scenario.refused] -->
<!-- r[related kiln.github.landing.merge_ancestry] r[related kiln.github.landing.merge_ancestry.scenario.advanced] -->
<!-- r[related kiln.github.source_status.scenario.open] -->
The accepted landing and closure criteria describe only refusal and read-only
observation for the ordinary GitHub route; neither path performs or claims an
unsupported mutation.

Ordinary GitHub PR-head-guarded merge does not atomically guard the base. `land` therefore refuses before transport; unsupported repositories/strategies stay PR-ready. There is no read-base/unguarded-merge fallback. A qualified protected merge-queue/provider route and real race/read-back evidence are prerequisites to enable unattended landing.

`recover-integration --publication-file FILE --landing-plan FILE --secret-file FILE` only observes external integration. It requires exact ordered base/head parents, reviewed tree mapping, applicable current checks/approvals and independent final-target ancestry. Closed-but-unmerged PRs and arbitrary merged flags cannot complete work. `VerifiedIntegrated` is the explicit landed terminal policy.

The landing admission refuses each stale or incomplete fact on its own, against an otherwise ready observation: a changed head, a moved base, another base ref or pull request, a closed or merged pull request, unknown or conflicting mergeability, a missing, running, failed, skipped or neutral required check, a check on another head, duplicate or unnamed checks, missing or author-only approval, a dismissed approval, changes requested by the latest review of any reviewer, and a malformed reviewer. A stricter current policy (more approvals, another required check or a newer expected base) refuses evidence admitted under the old one, and an approval bound to an earlier head is never reused. Branch protection is not observed: no landing route exists to bypass it.

`close-source --source-plan FILE --secret-file FILE` is now observation-only: it records an already closed exact issue observation under the plan's exact `expected_observed_revision`. It never PATCHes the issue or accepts any newer timestamp indiscriminately. An open or unadmitted source change remains unresolved. Conditional source mutation is unqualified.

`observe-cairn-lifecycle --attempt ID --fence N --binding-file FILE --repository PATH --receipt-path REPOSITORY_RELATIVE_PATH --authoritative-report FILE` reserves its own recoverable observation. The native receipt is read from the exact reviewed candidate Git tree and checked against the accepted integration tree, actual after-manifest blobs, authoritative completion artifact/policy and positive archive identities. It never runs post-review sync/archive or mutates protected source.

## Selected Cairn campaigns

The implementation owner starts one frozen `CampaignContract` with
`kiln-implementer campaign --database DB --command-file FILE`; the command file
uses `{"command":"start","contract":...}`. Inspect the same owner with an
`inspect` command to obtain its current `store_version`; `pause`, `continue`,
`cancel`, `issue_decision` and `resolve_decision` take that version. An
authenticated operator can do the same from the console through
[campaign controls](operator.md#campaign-controls-and-decision-inbox), which
start only configured, previewed definitions and record each command with its
request ID in the campaign database. A decision answer is refused once the
member's work, policy or candidate no longer matches the decision. The separate
native controller consumes the canonical Nickel
`implementation-campaign-drive.ncl` profile and an exact expected version:

```text
kiln-web-service drive-implementation-campaign --profile FILE --campaign-id ID --expected-version N
```

The bounded invocation requires existing campaign and implementation ledgers,
exact immutable role programs, a native Cairn source, canonical admission and
verification inputs, and one supported delivery route. It does not initialize
missing ledgers or fall back to another endpoint. Worker request and dispatch
inputs are templates; the controller binds the actual canonical work, attempt,
fence and source before native Lattice preparation and dispatch.

The declared parent budget must be an actual ancestor of the selected work's
canonical queue account. The owner rechecks this lineage and explicit admission
policy in the same SQLite writer transaction as the scoped claim. An overlapping
campaign reattaches the current attempt/fence, including a reserved capacity wait;
it does not mint another attempt or budget. A new claim is refused while another
active or unreconciled writer owns the same repository. Unrelated queue facts
remain intact when the campaign ranks only its selected work.

Source refresh observes the admitted remote/ref/commit and materializes a bounded,
root-owned, read-only Git snapshot for native Cairn reads. It rechecks the target
after collection and before stage admission; it does not reset the operator's
checkout. The controller's read-only target observer never inherits Git credential
helpers or an SSH agent. Anonymous public reads and explicit local fixtures are
supported; authenticated observation requires separately admitted scoped credential
delivery. GitHub delivery supports verified-candidate/verified-PR goals, not
automatic integration or lifecycle completion.

Decision projections retain their exact bytes and revisions under the canonical
attempt and projection digest before dispatch. A new context cannot silently
rewrite an in-flight request. Required checks are satisfied only by matching
passing check names in the canonical verifier result, not by the attempt's
generic verified phase.

Every selected native unit remains an obligation until its exact owner-observed
milestone. Empty readiness is not completion. Completion also requires current
candidate, policy and verification evidence and all required checks/reviews.
An observed, applied integration receipt can advance the source without erasing
its new identity; unrelated source, policy or decision-context drift requires
explicit replan. Pause/cancel closes new work and decision-request admission;
retained uncertain effects still reconcile and do not prove provider cleanup.

<!-- r[related kiln.implementation.campaign.adoption] -->
When native Cairn reports a selected change as completion-only, no coding work
is scheduled. The member adopts canonical evidence only when every required
native unit's current work has a latest attempt bound to it, independently
verified with exactly one passing `cairn-authoritative-completion` check, and
integrated through the selected route for that exact work, attempt and fence.
The integrated result must be contained in the current target, and the
change's `.cairn/changes/<change>` tree must be identical at that result and at
the target. The member then binds the attempt whose integrated result contains
every other, at its integrated stage, and the existing lifecycle step follows.
Any missing or stale fact keeps the member blocked with a reason naming it; a
change implemented outside Kiln, or only partly through it, stays blocked.

<!-- r[related kiln.implementation.campaign.independent_progress] -->
Selection evaluates every member. A required review, missing required context,
stale input, replan, unobserved or unsupported delivery capability or missing
endpoint evidence holds only its own member. Members that depend on it wait,
and independent eligible work proceeds in deterministic member order. A hold
becomes the controller's decision only when no member can act. The one-writer
repository bound remains effective when campaign concurrency is larger: artifact
authoring, coding, repair and retry wait while another attempt is claimed,
executing, a candidate or verifying. Canonical admission still decides each
claim, budget and parent lineage. When no member can act, the controller checks
every executing member for an ambiguous dispatch, not only the member whose hold
is surfaced.

The invocation prints the final `decision`, its `basis` (`campaign_barrier`,
`member_action`, `member_hold` or `no_eligible_member`) and `holds`: one retained
wait, blocker or replan per unsatisfied member, with its reason, owner and any
review request identity.

Maintenance owner: Kiln implementation/web-service maintainers; onix-core owns
the disabled-by-default, operator-started deployment wrapper. Scripted local
fixtures, real-model qualification and remote-effect qualification remain
separate evidence scopes.

<!-- r[related kiln.implementation.campaign.continuation] -->
### Continue a selected campaign

One drive stops when no admissible work remains. A supervisor resumes the
campaign with a one-shot continuation step over the same profile:

```text
kiln-web-service continue-implementation-campaign --profile FILE --campaign-id ID
```

A step takes no expected version. Before each batch it rereads canonical
campaign state and decides from it and a durable continuation record stored
beside the campaign:

1. Stop when the campaign is complete, cancelled with no effect left to
   reconcile, or when consecutive failures reach the failure budget.
2. Wait during a failure backoff, or for an operator while admission is paused
   and no effect is unresolved.
3. Drive after an operation-limit batch, when a wake hint is pending, to retry a
   failed batch after its backoff, on the first step, when the campaign version
   moved since the last batch, or when the rescan interval elapsed.
4. Otherwise wait until the rescan deadline.

A step runs at most `max_batches` driver batches of at most `maximum_operations`
operations each. It prints every batch (`reason`, `outcome`, steps, any claim
`refusal` or `error`) and the `next` step: `drive`, `wait` with `until_ms` (none
while paused), or `stop`. A `drive` result means the batch bound ended the step
with eligible work left. Run the step again at once for `drive`, at `until_ms`
for `wait`, or after a wake for `wait` without `until_ms`.

Batch outcomes are `operation_limit`, `waiting`, `awaiting_owner`,
`admission_refused`, `paused`, `needs_replan`, `cancelled`, `complete`,
`stale_version` and `failed`. Only `failed` counts against the budget, with a
doubling backoff capped at `retry_max_ms`. A `stale_version` rereads at once.
A canonical claim refusal is `admission_refused`: another work unit won the
exact admission or another active or unreconciled writer holds the repository.
It waits for a `capacity` wake or the rescan. Lineage, policy and currency
refusals stay driver failures.

An unresolved owner effect waits for its owner's wake, a version change or the
rescan instead of rereading on every step. It reconciles even while the campaign
is paused or cancelled. A cancelled campaign stops once nothing remains to
reconcile.

Owners and supervisors record wake hints with:

```text
kiln-web-service wake-implementation-campaign --database DB --campaign-id ID --kind KIND [--observed-version N]
```

`KIND` is `review`, `verification`, `source`, `capacity`, `reconciliation`,
`integration` or `operator`. Hints coalesce per campaign. Each one only asks the
next step to reread canonical state; it never grants an effect, a resource or
completion. A hint recorded while a batch runs remains pending afterwards. An
`operator` hint also clears the failure count and backoff. The campaign
operator's `resolve_decision` records a `review` hint and `continue` records an
`operator` hint in the same transaction as the transition. The command refuses
an uninitialized campaign and never creates a database.

The optional profile `continuation` record sets `rescan_interval_ms`,
`retry_base_ms`, `retry_max_ms` (each 1 s to 1 day, base not above maximum),
`max_consecutive_failures` (1 to 64) and `max_batches` (1 to 32). Without it the
named defaults apply: a 5-minute rescan, a 30-second base delay capped at 1 hour,
8 failures and 4 batches.

The continuation record holds demand, the last outcome, failures and backoff
only. Selection, claims, fences, effect intents and reconciliation stay with
their owners, so restarting the supervisor or the host loses no demand and
repeats no claim, dispatch, publication or integration. Two concurrent steps
can both reach the driver, but campaign compare-and-swap keeps each effect
single. Run one supervisor per campaign anyway.

## Qualification prerequisites and evidence

The baseline pilot needs one real reproducible bug and one feature, one writing worker per repository, immutable source/executable profiles, actual isolated workspace/kernel/network qualification, real model admission and operator-approved remote credentials. Cross-host two-worker/shared-model qualification additionally needs actual independent execution/inference leases, per-worker/service usage and cancellation/restart evidence; inventory snapshots and reservation-only accounting are not measurements or grants. Current authenticated-model credential delivery and unattended GitHub landing are explicitly unqualified.

Run the coordinated focused tests and real-step campaign only after integration, under the operator's exact approvals. Retain source/spec/base/candidate/profile/toolchain, native raw events, actual commands/artifacts, canonical runs, PR/integration and lifecycle identities. Report fixture evidence separately from real worker/model/remote effects. Empty ready sets, exhausted budgets, process exit zero and accepted candidates are not whole-project completion.
