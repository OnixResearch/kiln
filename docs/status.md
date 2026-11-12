# Implementation status and evidence index

This page carries the detailed ledger that the [README](../README.md)
summarizes: implemented areas, integration procedures with their retained
evidence, and the publication record. The text below was relocated from the
README without rewording; only relative link prefixes changed.

Every statement keeps the boundary of the document or receipt it links. A
retained receipt binds its recorded cohort and date. It does not validate
later source, and no entry here is a production, isolation or release claim.
For navigation by topic, use the [documentation index](README.md).

## Implemented areas

Kiln has an initial functional core and standalone reference shell. The implementation includes these areas:

- deterministic CI commands, events, effects, and replay;
- versioned provider capability admission;
- bounded read-only source snapshots from local Git and Pijul repositories,
  including a pinned native Pijul source-to-CI E2E rail;
- versioned Lattice workflow dispatch, lifecycle, controls, logs, and reconciliation;
- authenticated Iroh workflow transport through a private local Unix proxy and worker-local Lattice endpoint;
- exact Cairn gate request, process, result, and reconciliation admission;
- bounded isolated Patchgate review evidence with exact candidate/configuration admission, one-command atomic receipt production, a run-bound worker step, and retained-report reconciliation, separate from acceptance authority; see the [operator procedure](operator.md#isolated-patchgate-review);
- exact Mantle build request, bounded process, product, log, and reconciliation admission;
- exact stock and separately selected Onix Nix job discovery, bounded stream admission, task mapping, and GC-root handoff;
- bounded local `xyz.radworks.job` publication for one exact commit, with deterministic run UUID replay, start/finish actions, and multi-writer extension of an existing job; see [job publication](radicle-jobs.md);
- source, gating, fairness, task-operation, hosting, remote, interchange, and COB decisions;
- optional repository Atom feeds plus operator-managed external RSS/Atom subscriptions with a dedicated read-only token, DNS-pinned bounded HTTPS, and a console Feeds page; see [repository feeds](repository-feeds.md);
- packed Preserves and BLAKE3 canonical identity;
- bounded, deterministic, read-only observation projection over admitted state with typed profiles, paging and window admission, redaction, and `kiln.observe.view.v1` identities, including review-friction and suggestion-anchor views over self-declared, unverified change authorship;
- a bounded HTTPS observation shell with canonical JSON views, a bounded SSE event leg, Ed25519 or UCAN holder-invocation authentication, TLS, and no control surface;
- a bounded iroh observation shell for allowlisted peers with BLAKE3-verified payloads and maintained HTTP/iroh view equivalence;
- a local operator dashboard over a canonical Kiln journal or SQLite event store with recomputed event identities, canonical replay, bounded paged views, a SaaS-style offline workspace with overview/projects/runs/activity/evidence/settings, metadata search, per-history run filters and inspection, URL-restored navigation and appearance, a pinned Rats/Ratzilla repository/history index with accessible HTML navigation, read-only GitHub observation export, separately configured atomic HTML delivery, a source-reloading developer preview, and property-based terminal/browser rails without provider effects or hosted accounts;
- an independently packaged single-tenant HTTPS web workspace with run search and stable paging, URL-restored navigation, overview/timing, configured worker-target inventory and slot availability, dependency-graph inspection, continuous bounded worker logs and full-stream download, exact/latest/subgraph reruns, structured test/coverage/comparison and evidence views, durable queue/capacity/retention observations, signed branch- or tag-bound triggers, branch-bound schedules, configurable branch/tag/type/changed-path filters and calendar expressions in explicit time zones with hashed spreading, HTTPS notifications, operator-approved untrusted pull-request runs held in a durable blocked state and confined to declared untrusted-capable runners without secrets, workload token or cache, attributable viewer/operator access, and supervised deployment; see the [operator procedure](operator.md#single-tenant-live-web-service) for exact-profile, authority and evidence boundaries;
- a [Web → native Lattice → hosted execution route](hosted-ci.md#web-approval-through-lattice-to-hosted-execution), carrying persisted exact-source approvals through genuine saved workflows and a fixed hosted task, with independent Web-authority/Git verification and once-only backend recovery;
- an optional [internal developer loop](operator.md#internal-developer-loop) with source-bound per-step timing and resource observations, expiring authenticated worker capacity, fail-closed reviewed namespace isolation, verified shared cache transfers, and an HTTPS submit/watch/logs/cancel/retry CLI with explicitly selected content-addressed dirty inputs;
- an explicit offline [append-only project-registry extension](operator.md#extend-an-existing-project-registry) that preserves existing history and authority, plus a typed catalog of reviewed OnixResearch committed-ref CI commands;
- optional [monorepo project scopes](operator.md#monorepo-projects) for several projects on one repository, with a deterministic affected-project decision over exact tree diffs, declared dependencies and Cargo/Nix path graphs, conservative fallbacks, affected-only push/pull-request dispatch from each project root, and scope-bound run identities and per-project required CI in change review;
- an optional separately admitted [read-only COB browser](operator.md#optional-read-only-cob-browser) in the live workspace, with explicit local Git stores, six Cairn decision types plus three native Radicle observation types, typed list metadata, bounded inventory and exact raw-history inspection, without signature verification, lifecycle reduction or Radicle mutation;
- an optional [signed COB work workspace](operator.md#optional-signed-cob-workspace) with issues, planning, discussion, triage, principal-owned attention, durable private/shared views with Kanban boards that move only canonical issue status, monorepo-aware exact CI-project and repository filters, CI-project grouping and lanes, keyboard search/preview/bulk interaction and exact source/CI evidence joins; application collaboration is separately admitted and grants no CI operator or release authority;
- an optional [interactive change-review workspace](operator.md#optional-interactive-change-review) with stable branch, Jujutsu change-ID and stack contribution intake, patch sets, syntax-highlighted exact diffs with rebase-aware interdiff and hunk-level viewed markers, a syntax-aware structural mode, safe Markdown, whitespace comparisons, display-only noise globs for generated, vendored and lock files, keyboard navigation and actions with a help overlay, private atomic publication, immutable anchored suggestions, path/team ownership, revision-bound approvals, non-binding advisory approvals that never count, personal attention and bounded external HTTPS delivery; exact-candidate required-CI merge automation, including a separate speculative merge queue, uses durable provider journals and target CAS, independently of retained Patchgate receipts and deployment-level branch protection;

- optional review ownership evidence with project-CI binding, ordered change classes and explicit evidence-only approval, interface-scoped dependent CI and owner acknowledgement with policy-declared dependents, advisory or binding agent vote policy, attested agent submissions with declared harness/model metadata, agent identity in sessions and audit records, and read-only agent-attestation, declared-harness and declared-model review search;
- an exact Aspen/Molten runtime profile, atomic event-store mapping, bounded content and callback contracts, and fail-closed ingress;
- a standalone `kiln-aspen-host` composition shell that runs the real Aspen native service and concrete Lattice effect adapter;
- exact-cohort host qualification receipt admission, retained-artifact identity checks, and stronger-claim rejection;
- a standalone `kiln-radicle-nix-provider` shell for bounded checks and durable reports under Lattice;
- an exact local Nix derivation build provider with discovery-task composition, pre-dispatch derivation admission, typed constraints, NAR/reference and input-closure facts, durable state, rooted outputs, bounded logs, reconciliation, and a read-only cache projection;
- an isolated signed-operator OnixOS pilot joining the maintained Aspen/Lattice runtime to source-bound discovery, rooted application builds, canonical journal replay and UCAN-authenticated observation;
- Radicle broker compatibility, exact signed-reference source acquisition, bounded CI status publication, and native multi-writer patch reviews through an isolated pinned upstream helper;
- a fixed-profile Radicle source-event composition with verified materialization, real Nix discovery/build, canonical recovery and exact-revision status reconciliation, including exact branch or canonical tag sources and tag-triggered CI; plus separately selected revision-bound patch approval/journals and authenticated hosted execution without privileged untrusted grants;
- authenticated GitHub push/PR intake with bounded single-pass or interval polling recovery, exact HTTPS source jobs through reviewed Lattice workflows, durable lifecycle recovery, and bounded commit-status publication;
- a bounded GitHub/Cairn implementation-work ledger with claims, fences, dispatch reservations/plans, candidate handoff, canonical CI verification, metrics, verified branch/PR publication, read-only landing evidence with approvals and exact base state, guarded merge-commit landing with read-back recovery, integration receipts, and idempotent integrated-source closure; see the [implementation runbook](implementation-runbook.md);
- authenticated Gitea/Forgejo push/PR intake, exact HTTPS source jobs, durable replay, and bounded commit-status publication through the separate `kiln-adapter-gitea` shell;
- a [Tangled spindle-compatible service](tangled-integration.md) with service-auth-verified knot pushes, exact-commit acquisition, a reviewed `.tangled/workflows` subset, untrusted pull request approval and signed status projection; live knot interoperability is not yet established;
- an optional [Iroh-connected hosted CI composition](hosted-ci.md) with scoped accounts, upstream repository connections, financial admission, reviewed pipelines, explicit execution/fleet backends (including a [per-job Firecracker microVM backend](hosted-ci.md#6-explicit-isolation-resource-limits-and-execution-recovery) that moves jobs off the host kernel, an opt-in bounded debug hold for failed jobs, and operator-declared untrusted-capable runners), workload identity and local recovery; this is not production or hostile-isolation qualification;
- typed Nickel configuration and Nix checks.

## Integration procedures and retained evidence

Onix Nickel/WASM discovery is an explicit optional profile, not the stock default.
Provision its separate pinned cohort and run the real discovery rail using the
[Onix procedure](nix-eval-jobs-adapter.md#explicit-optional-onix-profile). The
[immutable fixture receipt](../runtime/onix-eval-cohort/jobset.source.json) records
the imported OnixOS source. Core and stock-only builds do not acquire the Onix
evaluator or plugin; passing discovery does not prove a build or deployment.

The [isolated maintained-cohort OnixOS pilot](operator.md#isolated-maintained-cohort-onixos-pilot)
exercises authenticated construction of the real launcher, activation script
and generated home files, duplicate delivery, host restart/reconciliation and
genuine evaluation/home-file build failures. Only external tools are prepared
separately; all three consumer outputs must be absent before dispatch. The pilot
neither executes activation nor establishes autonomous forge delivery,
production cutover or release authority. The
[full-closure qualification](../.cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/verification.json)
retains the measured construction boundary and repeatability evidence.
The [historical launcher-only qualification](../.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/verification.json)
retains its original narrower claim and does not qualify the new construction rail.

The [Radicle source-to-build composition](operator.md#radicle-source-to-build-composition)
accepts an admitted push without fabricating an operator-signed pilot request.
Its fixed reviewed workflow builds the verified event source and retains a
revision-bound status intent. The native proof uses the real `cibtool` producer
against isolated signed-source and status fixtures; it is not a live remote
Radicle delivery or production rollout.
The [retained qualification receipts](../.cairn/archive/1970-01-01-compose-radicle-source-to-build-ci/evidence/verification.json)
bind the exercised source, tools, recovery, status and retirement observations.
The status and patch-review shells use the separately pinned v0.2.0 shared Git COB store imported into this repository, not a sibling checkout. See `docs/radicle-status.md` for its immutable source digest and drift-refusing import procedure.

For GitHub CI, use the typed builders `config/github-ci.ncl` and
`config/github-job.ncl` with real operator-supplied repository IDs, SecretSpec
references, executable/profile inputs, and exact reviewed workflow identities.
The default package installs `kiln-adapter-github` and `kiln-github-job`, also
available as Nix apps. Follow the [end-to-end operator setup](operator.md#github-ci-adapter)
for HTTPS webhook ingress, Lattice `route_exact`/`shell_environment` routing,
job JSON byte pinning, permissions, status, and recovery. `fixtures/github/`
contains isolated synthetic conformance inputs, not production configuration.
Kiln maintainers own this integration's repeatability rail; operators retain
deployment, credential, worker isolation, and repository authority. Local checks
do not prove live deployment, remote status writes, source trust, or confinement.
Recorded local outcomes, executable identities, and scoped producer-gate results are in
the [scoped GitHub CI evidence](../evidence/github-ci-local-runtime.ncl).

For Gitea and Forgejo CI, use `config/gitea-ci.ncl` and `config/gitea-job.ncl`
with the normalized instance API and Git bases, numeric repository id,
SecretSpec references, and a reviewed workflow identity. The default package
installs `kiln-adapter-gitea` and `kiln-gitea-job`, also available as Nix apps.
Follow [the Gitea/Forgejo procedure](gitea-ci.md) for webhook headers,
status permissions, exact-source jobs, recovery, and the explicit scope
boundary. The optional `kiln-hosted` connection composition remains
GitHub-only in this change.

For remote workflow execution over Iroh, use `kiln-adapter-iroh` with the existing
consumer's Lattice socket setting. Follow [peer enrollment, private keys, profiles,
startup, and recovery](operator.md#iroh-remote-workflow-execution). The
[change-local evidence](../.cairn/archive/1970-01-01-add-iroh-workflow-transport/evidence/verification.json)
records actual local execution and exact recovery after both bridges restart,
not WAN/relay availability or deployment authority.

The [combined live CI proof](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json)
joins genuine GitHub deliveries to exact-source compile/test jobs on a separate
Iroh worker, canonical passing/failing results, and matching Published GitHub
statuses. It also records controller/bridge restart recovery of the original
still-running execution and genuine redelivery suppression. Follow the
[bounded live exercise](operator.md#bounded-combined-live-ci-exercise) for
the recorded cohort, repetition, credential retirement, and claim limits.

The [live Radicle CI canary](../.cairn/archive/1970-01-01-qualify-live-radicle-ci/evidence/verification.json)
starts at a genuine native peer update, lets the installed broker's event
subscription invoke the unchanged controller/worker, and independently reads the
dedicated exact-revision status from observer storage. It records canonical
`Success` and source-controlled `DomainFailure`, an in-flight execution surviving
host kill and reconciliation without redispatch, duplicate-fetch suppression, and
bounded nonterminal uncertainty when the provider is unavailable. Follow the
[native Radicle peer and broker canary](operator.md#native-radicle-peer-and-broker-canary)
for the recorded cohort, repeat command, retirement, and claim limits.

The [distinct-host Radicle canary](../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-ci/evidence/verification.json)
moves only the publisher node and source worktree to a second owned host while
the CI node, broker, controller, worker and observer stay local. Source delivery
crossed a direct private-tailnet TCP connection; success, genuine failure,
controller host-kill recovery, remote publisher restart and unavailable-provider
uncertainty all passed. Status dissemination remained same-host; the run is not
WAN, hostile-network or production qualification. See the
[distinct-host procedure](operator.md#distinct-host-publisher-canary).

The [distinct-host status canary](../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-status/evidence/verification.json)
moves the observer node and status storage to the second host while status
creation and signing stay local. The remote observer fetched and reduced the
exact success/failure statuses with the pinned read path, and the unavailable
case retained no status. This is private-tailnet dissemination and readback,
not WAN, hostile-node isolation or production qualification. See the
[status observer procedure](operator.md#distinct-host-status-observer).

## Publication record

Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE`. Use its signed Radicle refs for current branch identities. The historical operator-authorized publication receipt covers commit `36984e367eafd44c22d2b4f4f47121deb51eece3`; it does not validate a later branch tip or changed local source. The later source-only cutover to `f748acd0cc239f48bf3565ccb89d46ce88cbee2f` and its candidate-bound checks, including the optional runtime attributes that were not exercised, are recorded in [`.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/`](../.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/publication.json). See `docs/hosting-and-recovery.md` for candidate and acquisition boundaries.

The subsequent operator-authorized main cutover to
`6c6e4f220fe0234ce607085fa622b580fd1cd016` has its own
[source/acquisition receipt](../.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/main-cutover/receipt.json).
The pilot's frozen-cohort qualification is independent of later source
publication; publishing these sources does not qualify changed executables.

The operator-authorized [pilot source publication](../.cairn/archive/1970-01-01-prepare-public-release/evidence/maintained-pilot-publication-2026-10-20/publication.json)
records source `f09aa7187dff390272b9a8bcca3a9be90f71e6b9`, matching native and
anonymous HTTPS acquisitions, unchanged governance and its exact archive.
The publication receipt is a later evidence-only descendant, not a new runtime
qualification.

The operator-authorized [qualified-source publication](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/publication.json)
fast-forwarded canonical Radicle `main` from `305f85d80b9ea05fbd91db73de8a6662a55650ee`
to the self-CI-qualified source `c955334f54bdfe6e155e56c00573ca7ba77c4fb1` with
no force push, history rewrite, governance change, or mirror mutation. Its
deterministic source archive is BLAKE3
`28fec0fa2b3a71c4c4e725d4640080a07d18d4a5929ada05d7cd9b81eff38004`
(60,989,440 bytes, 4,439 tracked members) and its sorted path manifest is
`8001e6b5cbc334cdb1d46611e71d824a4deee82d399937983a19743c75a45383`. Native
Radicle and anonymous Garden HTTPS acquisitions both reproduced that revision,
path manifest and archive, the delegate set and threshold one were unchanged,
and the GitHub projection was observed but not mutated. The receipt is a later
evidence-only descendant of the published source.

The subsequent operator-authorized
[evidence-revision publication](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/evidence-revision-publication-2026-10-21/publication.json)
fast-forwarded canonical Radicle `main` to the evidence revision
`acc359e769bb3a056279bf5db8ba21e95c294a4c` and advanced the read-only GitHub
projection to the same revision, with no force push, history rewrite, governance
change, or deployment. Its source archive is BLAKE3
`1f72af29b005a7162c45b4fbd60ba94c19698451663e1285c91f6b2496e69a9a`
(61,480,960 bytes, 4,457 tracked members) and its sorted path manifest is
`e97163d1367f5c76e6541683f06266f9866f28b2261ecb9ee204e19f4a4e7a83`. Native
Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS all reproduced that
revision, path manifest and archive, and a credential-isolated anonymous clone
passed the workspace suite. The delegate set, threshold one, and public
visibility were unchanged.

The subsequent operator-authorized
[evidence-commit publication](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/evidence-commit-publication-2026-10-21/publication.json)
fast-forwarded canonical Radicle `main` and the read-only GitHub projection to
the evidence commit `acacf2b38f04f462f588215aa7e314e3cdc4ca04`, which carries
the `acc359e7` publication receipt, with no force push, history rewrite,
governance change, or deployment. Its source archive is BLAKE3
`840226ed3bcdd9671d26cb5f5e334c49be7270e6ba5fe47040d87e81d7225513`
(61,890,560 bytes, 4,465 tracked members) and its sorted path manifest is
`6342b8de98a20f992673272f40ca275ed0af5ab91cc339c5223811c68db4badc`. Native
Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS all reproduced that
revision, path manifest and archive, the Rosa repository API reported the same
`refs/heads/main`, and a credential-isolated anonymous clone passed the
workspace suite with 527 tests and no failures. The delegate set, threshold
one, and public visibility were unchanged.

The further operator-authorized
[final-evidence publication](../.cairn/archive/1970-01-01-publish-qualified-source-20261021/evidence/final-evidence-publication-2026-10-21/publication.json)
fast-forwarded both remotes to `0019ff3cbe1abbc2cca1b109b539be719629cd15`,
which carries the `acacf2b3` receipt, with the same unchanged governance and
deployment boundary. Its source archive is BLAKE3
`720a251e64e75e0e3606d55574745346801f7b14b2caa13ac438dbfbdacd6eaa`
(62,300,160 bytes, 4,473 tracked members) and its sorted path manifest is
`dc9695653796cfb5905a437506eab9b90fe4205309abd8f237e64e83cd6b8752`. All three
required routes reproduced that revision, path manifest and archive, and a
credential-isolated anonymous clone passed the workspace suite with 527 tests
and no failures. Each publication receipt is carried by the revision that
follows it, so the receipt for the current tip lands in the next revision.
