# Implementation service runbook

Kiln owns work, current claims/fences, cross-attempt budget, independent acceptance and publication. Lattice executes one bounded attempt. The supported consumer is the fixed trusted dispatcher, the canonical Engine verifier and the separately credentialed publisher—not a worker success manifest. Kiln maintainers own these commands; the operator owns source/profile enrollment, physical/model grants and deployment.

This is an implementation procedure, not a live qualification receipt. No real bug/feature, OMP/model call, GitHub publication, landing or lifecycle mutation is claimed here. The executable campaign and evidence requirements are in [implementation qualification](implementation-qualification.md).

## Intake, policy and immutable preparation

Admit authenticated GitHub issue deliveries using `kiln-implementer drain`/`run` or `poll`/`poll-run`, or use `kiln-cairn-intake` for an actual complete implementation-readiness snapshot. Policies supply obligations, effects, source aliases, exact Git base, attempt budget and terminal policy; issue prose does not grant effects. Duplicate/stale intake remains idempotent. New source versions atomically apply supersession; `admit_with_supersession` exposes explicit abort/finish policy without rewriting the frozen contract.

Default operator intake selects `AbortActive`. When an embedder explicitly selects
`FinishActive`, capture and canonical verification use the single transactional
`Implementer::active_work` accessor: a historical frozen contract is returned only
for the exact still-current, active, unexpired attempt/fence whose retained
`superseded_by` matches the newer current revision. Cancelled, aborted, replaced,
expired or unproved historical attempts fail closed. Superseded candidates never
gain publication authority; the newer source remains separately admitted work.
This is the supported source/fixture path, not a claim of an exercised live pilot.

Human-authored policy remains typed Nickel (`config/implementer-policy.ncl`, `config/implementation-worker.ncl`). Export operator-reviewed inputs before passing JSON to bounded CLI consumers.

Prepare the verifier profile through its native codec:

```console
kiln-implementer prepare-verification-profile \
  --profile /etc/kiln/verification-seed.json \
  --output /var/lib/kiln/verification-profile.json
```

The output reports the canonical profile identity and `checks_executed:false`. Use that identity in intake policy. Preparation computes frozen toolchain/environment identities; it does not run checks or establish acceptance.

Configure opt-in bounded fair admission with `kiln-implementer admission --command-file FILE`; `admission-status` reads its retained ledger. Commands configure policy/accounts, enqueue, select/claim, retain owner reservations/replies and settle observed usage. `--admission-observations FILE` supplies fresh provider facts to actual dispatch. Selection, inventory eligibility, a configured worker and a model deployment are never grants. Unknown usage remains conservatively charged.

`claim` is the existing configured-route compatibility consumer. For the independently bound worker route, use admission's claim command so the queue/account and current attempt are associated. `inspect-work --work ID` returns exact retained work, source versions and native acceptance digest.

## One real bounded worker attempt

Prepare an isolated workspace from the exact admitted Git base. No linked `.git` pointer, publisher credentials, verifier-private material or database path is exposed to the coding child. The selected runtime uses the already implemented bubblewrap OMP route. It must have a reviewed OMP build at revision `e325ffc73ab86e03bdd20a91b77d8e2b3964c906`, exact executable BLAKE3 and the selected kernel/provider qualification.

```console
lattice implementation-attempt prepare \
  --request /var/lib/kiln/worker-seed.json \
  --output /var/lib/kiln/worker-request.json
```

The native producer reads the actual baseline snapshot, computes its own snapshot/profile/envelope/request identities and writes a new frozen request. Its strict schema is `lattice.implementation-attempt-run.v1`; the implementation is `src/implementation_attempt/cli.rs` in the explicitly adopted Lattice source cohort. Seed envelope work/version/attempt/fence, source native ID/version, acceptance digest, exact repository/base and verification profile come from actual Kiln observations. Preparation is not admission or execution.

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

Transport schema `kiln.lattice-implementation-cli.v1` binds immutable `lattice_program`, `executable_blake3`, `state_directory` and private `output_directory`. It runs only the selected immutable executable. Current work/fence, frozen request bytes, source/spec/profile/workspace bindings, queued runtime deadline and fresh owner grant generations are checked before transport. Selected plans/bindings and dispatch intent are durable. A second invocation automatically calls native recovery; explicit `--recover true` refuses absence of a prior transport intent.

Use the actual trusted dispatcher host UID (for example 931) in verifier authority. A unit-only ledger group belongs to the fixed trusted dispatcher, not the sandboxed OMP program: the coding child sees only its explicit private-root mounts. Do not pretend a child changed host UID or grant it publisher credentials. Separate verifier UID (for example 933) and private acquisition/scratch roots are required.

A network-enabled request must name the backend-owned `/run/netns/NAME`; Lattice checks that its current namespace has the exact selected namespace identity. No namespace is created or egress policy inferred. The backend/operator must qualify endpoint-scoped egress. The current OMP route has no credential-delivery provider: authenticated model deployments remain unsupported. Authless `.omp/agent/models.json` configuration must already be part of the reviewed recorded baseline; it is not silently excluded from source capture or injected after baseline hashing.

Native output retains `artifacts.raw_events`, `artifacts.omp_observation`, envelope and actual candidate receipt. Cancellation via SIGINT/SIGTERM is forwarded by the coordinator to `lattice implementation-attempt cancel --request FILE --state DIR`; Lattice persists an exact request and invokes its existing process-group cancellation. Requested cancellation is not observed termination. Unknown teardown/controller loss quarantines the workspace and preserves debit; no automatic release, refund or successor reuse follows.

`kiln-implementer recover` reports expiry and pending reconciliation, always with `project_complete:false`. `reconcile-termination` accepts an explicit operator/provider termination-reference observation; a timeout or missing process is not such evidence. Expired/cancelled/replaced fences cannot accept later candidates.

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

## Publication, integration and source reconciliation

`prepare-publication --publication-file FILE` freezes the exact verified candidate, work/version/fence, branch and concise source/change/validation/limitations description. `publish --publication-file FILE --secret-file FILE` reserves branch and PR effects separately before transport, independently reads back each result and acknowledges atomically. Lost responses grant read-only reconciliation, never blind duplicate creation. Unexpected heads, all-state PR ambiguity and incomplete pagination refuse progress. Verified-PR policy leaves source work open.

Branch creation requires the exact candidate Git object to exist on the remote.
GitHub `POST /git/refs` creates a reference; it does not upload local Git objects.
Qualification must retain an explicitly approved Git/object publication stage or
an independent existing-object observation before branch/PR publication. A local
candidate SHA or a fixture accepting that SHA is not remote-object evidence.

<!-- r[related kiln.github.landing.merge.scenario.landed] r[related kiln.github.landing.merge.scenario.refused] -->
<!-- r[related kiln.github.landing.merge_ancestry] r[related kiln.github.landing.merge_ancestry.scenario.advanced] -->
<!-- r[related kiln.github.source_status.scenario.close] -->
These accepted landing/closure criteria remain unqualified for the ordinary
GitHub route; the following refusal and read-only observation paths do not
claim that an unsupported mutation was performed.

Ordinary GitHub PR-head-guarded merge does not atomically guard the base. `land` therefore refuses before transport; unsupported repositories/strategies stay PR-ready. There is no read-base/unguarded-merge fallback. A qualified protected merge-queue/provider route and real race/read-back evidence are prerequisites to enable unattended landing.

`recover-integration --publication-file FILE --landing-plan FILE --secret-file FILE` only observes external integration. It requires exact ordered base/head parents, reviewed tree mapping, applicable current checks/approvals and independent final-target ancestry. Closed-but-unmerged PRs and arbitrary merged flags cannot complete work. `VerifiedIntegrated` is the explicit landed terminal policy.

`close-source --source-plan FILE --secret-file FILE` is now observation-only: it records an already closed exact issue observation under the plan's exact `expected_observed_revision`. It never PATCHes the issue or accepts any newer timestamp indiscriminately. An open or unadmitted source change remains unresolved. Conditional source mutation is unqualified.

`observe-cairn-lifecycle --attempt ID --fence N --binding-file FILE --repository PATH --receipt-path REPOSITORY_RELATIVE_PATH --authoritative-report FILE` reserves its own recoverable observation. The native receipt is read from the exact reviewed candidate Git tree and checked against the accepted integration tree, actual after-manifest blobs, authoritative completion artifact/policy and positive archive identities. It never runs post-review sync/archive or mutates protected source.

## Qualification prerequisites and evidence

The baseline pilot needs one real reproducible bug and one feature, one writing worker per repository, immutable source/executable profiles, actual isolated workspace/kernel/network qualification, real model admission and operator-approved remote credentials. Cross-host two-worker/shared-model qualification additionally needs actual independent execution/inference leases, per-worker/service usage and cancellation/restart evidence; inventory snapshots and reservation-only accounting are not measurements or grants. Current authenticated-model credential delivery and unattended GitHub landing are explicitly unqualified.

Run the coordinated focused tests and real-step campaign only after integration, under the operator's exact approvals. Retain source/spec/base/candidate/profile/toolchain, native raw events, actual commands/artifacts, canonical runs, PR/integration and lifecycle identities. Report fixture evidence separately from real worker/model/remote effects. Empty ready sets, exhausted budgets, process exit zero and accepted candidates are not whole-project completion.
