# Changelog

All notable changes to Kiln are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions are the
workspace version in `Cargo.toml`.

Entries describe implemented mechanisms. A passing Kiln decision proves the
checked relation between one admitted state, command, event set and effect set;
it does not prove provider behavior, source trust, build correctness or release
eligibility.

## [0.1.0] - unreleased

First public version. No tag or release artifact exists yet on any remote. This
entry describes the source tree prepared for the first tagged release.

### Added

**Core**

- Closed implementation-authority coverage: every protected claim, renewal,
  dispatch, candidate submission, verification admission, cancellation,
  publication, integration and lifecycle path binds one reviewed Basalt
  operation, enforcement boundary, effect class and candidate rule. Verified
  holder invocations or an explicit local-operator profile are required before
  any mutation, provider call or credential resolution; every effect revalidates
  without consuming replay twice, and bounded redacted decisions retain no
  capability material. Basalt is imported at `264d248` with recorded UCAN path
  rewrites, UCAN is one snapshot at `417f746`, and a metadata-only
  `detect-coding-agent` 0.1.1 declaration patch, added for the former Rust 1.88
  pin, remains in place.
  (#69; adapter, implementer, policy and denial fixtures)
- Attempt grant lineage core (`kiln.attempt-authority-binding.v1`): a root
  binding ties an admitted ceiling to the exact campaign revision, work
  version, attempt, fence, holder and #61 reservation, and an admitted parent
  derives a child only under explicit delegation permission with no wider
  resource, operation, audience, lifetime, disclosure route or depth, and no
  role crossing from coding into verification or publication. Every refusal is
  one closed `LineageRefusal`; revocation, expiry, a moved generation or a
  replaced fence on any ancestor invalidates every descendant. Ceiling, intent
  and binding identities are deterministic BLAKE3, and a binding holds proof
  references only. Attempts retain one optional binding, absent from
  serialization until bound; an unwired issuance fold resumes an unissued
  intent under its original identity and propagates revocation. Verified
  cross-holder delegation waits for Basalt B1. (#70; pure-core fixtures)
- Pure `no_std + alloc` `kiln-core` implements both transitions:
  `state + command -> events + requested effects` and
  `state + event -> new state`, with no filesystem, process, network, clock or
  credential dependency. (workspace tests)
- Canonical journal replay, packed Preserves encoding and BLAKE3 identities for
  domain artifacts. (workspace tests)
- Versioned ports for source, collaboration, discovery, workflow, gate, remote
  execution and trigger intake, plus a shared adapter conformance crate.
  (fixture-tested)
- Typed provider failures keep timeout, cancellation and unknown distinct until
  an application shell applies retry policy; expired or ambiguous leases enter
  reconciliation. (workspace tests)
- Opt-in implementation-attempt outcome and effort reports retain worker
  terminal state separately from controller verification, with explicit
  absent/incomplete model-request counts. A claim-time window reports rejected
  Completed shares, disjoint failures, turns and wall time per verified pass,
  sample coverage and profile/model groups without ranking or dollar pricing.
  Existing verification/publication authority and first metric receipts remain
  unchanged; current OMP JSONL events are not model turns. (#90; deterministic
  core, durable store, adapter and operator CLI fixtures)
- Opt-in serving backpressure and recursive budget lineage for fair
  admission. A provider-neutral serving-owner snapshot (the Animus #17
  availability shape) holds worker-request work before claim and before each
  request as `overloaded`, `stale`, `waiting`, `incomplete` or `incompatible`;
  a snapshot never reserves, replaces the owner grant or refunds. Lineage
  revocation and cancelled or expired parent attempts block new descendant
  queueing, requests, transfers and dispatch without refunds, and a
  conservation-checked lineage view keeps available, tentative, held, unknown,
  settled and rejected usage apart. Serving policy and recursive budget
  profiles are typed Nickel with checked committed exports. Defaults and
  existing policy identities are unchanged. (#61; deterministic core and
  durable-store fixtures with injected owners; no live serving, capacity or
  cross-host qualification)
- `kiln-lattice-dispatch` sends Lattice's `lattice.kiln-budget-lineage.v1`
  (Lattice `5a8e1d51`). The lineage is derived from the admission ledger:
  the campaign root, the work's account, and the attempt's committed owner
  reservations as its remainder, once every required owner has granted.
  Every dimension Lattice charges that the account tracks must be held;
  other dimensions are sent when held, for Lattice to list as unaccounted,
  and omitted otherwise, as for a shared allocation's `gpu`. The
  `--worker-request` route appends the lineage only for an admitted
  attempt, and keeps every admitted byte. Without admission, requests keep
  their bytes and identities. The sent bytes are frozen atomically under
  their own BLAKE3, so a retry refused before any transport start is never
  blocked by an earlier or torn file. That file's path and digest are
  recorded for the launch, in the transport intent's transaction, under a
  separate `transport_requests` record, so the launch record keeps its
  published shape and an older binary still reads it. Recovery and campaign
  export read exactly the recorded file, whatever the profile's output
  directory. The `--dispatch-plan` exchange
  route checks the same lineage, but its `lattice.workflow-request` cannot
  carry it. An unsettled or crossed reservation, a missing required grant,
  an inconsistent ledger, a blocker, backpressure, an exhausted or
  unrepresentable lineage, or a request that already names one is refused
  with a typed `LineageRefusal` before the owner-grant gate and transport
  start. The qualification crash barrier finds Lattice's intent by the sent
  digest. (Lattice #6; golden bytes, Lattice's published schema as data and
  durable-store fixtures; no live Lattice worker)
- Default-off, exact-revision session-trace pointer observations freeze locally
  admitted Git notes in the existing review journal. BLAKE3 pointer/note
  identities remain distinct from external IDs and URL hosts; missing,
  incompatible and unavailable notes stay explicit. The selected revision's
  inert untrusted-context card grants no authorship, CI, ownership or merge
  authority; no session bodies, remote fetches or URL secrets are retained.
  (#91; pure-core, local-Git, durable-journal and typed-profile fixtures)
- Default-off renews/iroh swarm claim mirror: pure claim/release rules with
  redacted bodies and a 24-hour overlap rule, a bounded `iroh-news` client whose
  posts classify as accepted, rejected, unknown or unavailable, an atomic
  per-fence journal that never repeats a transition, and the `kiln-swarm`
  read/post CLI. No credentials enter Kiln; swarm content can only defer.
  (#94; core, adapter and CLI fixtures)
- Default-off fleet orchestration of interactive agents (`kiln-fleet`): a
  pure fleet core (claim scopes with containment overlap, fenced and leased
  fleet claims that conflict with other claims, live attempts and foreign
  swarm claims, an interactive session ledger with a pure tick planner,
  operator decisions and attempt prompts), durable fleet state in the
  implementation store, the `kiln-adapter-herdr` session provider, explicit
  `interactive_worktree` dispatch plans for `herdr-interactive` work that
  permits `interactive_agent`, herdr candidate receipts for
  `materialize_candidate`, and one service with a Tailscale-authenticated
  JSON API, a stdio MCP server, client commands and the dispatch loop. The
  swarm mirror now reads the newest window, posts with Kiln's own From
  address and defers by containment. (#96; core, store, adapter and service
  fixtures)
- Fleet observation and operator surfaces: the console Fleet page keeps one
  persistent SSE stream of claims, ranked/blocked work, attempts and open
  decisions; the selected Cairn change shows a read-only inline fleet panel
  joined by exact work identity; `kiln-fleet status` emits JSON, Waybar or tmux
  counts, `kiln-fleet watch` renders a live ANSI dashboard with line commands,
  `/v1/metrics` exposes Prometheus gauges, and the MCP server exposes read-only
  `kiln-fleet://` resources. Commands still pass the fleet service's own
  authority checks. (route, profile, service, CLI and console tests)

**Sources**

- Bounded read-only snapshots from local Git, Pijul and Jujutsu (Git backend).
  (Pijul rail exercised locally; Git and Jujutsu fixture-tested)
- Exact Radicle signed-reference acquisition and admission through a pure source
  contract that carries no provider runtime. (fixture-tested)
- A pure source projection contract: a repository identity bound to the
  provider-native reference instead of a checkout path, Nickel-authored
  navigation, runnable and writable definitions, and frozen manifests that map
  every destination to a native or generated origin. Partial views and
  completeness witnesses are distinct types, and a stored manifest can be
  checked for tampering and staleness but never admitted. Monorepo scopes
  convert through a labeled compatibility profile. (#75; unit tests, doctests
  and Nickel fixtures; no consumer or authority wiring yet)
- Materialize a complete source projection into a verified immutable input
  and private per-attempt writable overlays. A pure plan derives a reusable
  input identity from the manifest and materializer profile and an overlay
  identity from the input, attempt and fence, refusing symbolic links,
  generated entries and platforms that cannot represent the target. The local
  shell verifies each Git blob, publishes inputs by no-replace rename before
  binding them, re-verifies reused inputs, and never reuses or overwrites a
  directory without its matching binding. An implementation dispatch plan can
  pin the projected workspace (repository, exact source, definition, manifest,
  materializer profile, input and overlay generation for its attempt and
  fence); admission refuses a crossed pin, and the worker resumes an overlay
  only under the same retained pin. Plans without a pin keep their identity.
  (#78; unit, dispatch and temporary-repository tests; no worker job, authority
  or restricted-runtime wiring yet)
- A typed shared corpus (two projects, a shared dependency, an executable file,
  equal-byte files at two origins and an excluded entry) runs through the eager
  local materializer for two attempts in the off-by-default
  `workspace-baseline` example. Its receipt records nine contract checks and
  the timings, and a test compares it with a committed expectation apart from
  timings. (#78; Nickel fixtures and a frozen receipt)
- Plan the source inputs of a projected build. Versioned
  `cargo_compatibility.v1` and `nix_compatibility.v1` profiles require, at
  their own paths, every file of the target packages, of the workspace members
  Cargo loads and of their path-dependency closure, plus the workspace
  manifest, lockfile, Cargo configuration and toolchain files; restricted Nix
  views are refused because sibling reads are invisible. Relocated or generated
  entries, unknown or ambiguous targets, outside path dependencies and absent
  inputs are refused before anything runs. The plan identity binds inputs,
  command, toolchain and environment, and evidence applies only to the same
  identity. `kiln_build_graph::cargo_layout` supplies workspace membership and
  external dependencies. A typed two-project Cargo corpus is planned and
  materialized by the off-by-default `build-input-qualification` example. With
  the `build-input-fixtures` feature its commands run under a cleared
  environment: one view passes, one with an undeclared cross-package read
  fails, and a shared-dependency edit selects the project outside the view.
  (#77; unit, layout and frozen-receipt tests and a Nix check; no worker job,
  relocation or authority wiring yet)
- Capture and seal edits made in a writable projected workspace. A pure
  capture maps each overlay file back through the frozen manifest and the
  definition's mounts to one typed `edit`, `create` or `delete` naming its
  expected base object. It refuses disagreeing aliases, new files outside
  creation destinations, ambiguous new-file routing, creates over hidden
  source files or outside the selection, symbolic links, a moved source, a
  superseded fence and a pin for another view. The sealed candidate has one
  BLAKE3 identity and a pure reconstruction. The worker shell hashes overlay
  files as Git blobs, re-reads them at sealing and refuses any that changed
  after capture, and commits a tree that differs from the base at exactly the
  operation paths, without moving a reference. (#79; unit and
  temporary-repository round-trip tests; no verification, publication,
  rebase or authority wiring yet)
- An optional graph-backed source model in pure `kiln-core`. File and package
  identities are minted once by their creating operation, never from bytes or
  paths; immutable revisions and snapshot roots are separate identities.
  Memberships, aliases, copies, forks and pinned dependencies are versioned
  records, and membership removal, alias removal and deletion have distinct
  outcomes. Imports keep an entity only through its recorded path or an
  explicit mapping and record other additions as unresolved continuity;
  deterministic exports preserve bytes, executable modes and deletions. One
  authority mode per repository refuses stale or independent publications
  until reconciled, and evidence names every changed identity. A prototype
  reproduces one snapshot, two layouts and a shared entity after restart.
  (#81; unit and prototype fixtures; no store, provider adapter, repository
  conversion or publication)

**Forges and triggers**

- GitHub push and pull-request intake, polling recovery, exact HTTPS source
  jobs and commit statuses. (exercised live once; no persistent deployment)
- Gitea/Forgejo signed webhooks, exact-source jobs, durable replay and commit
  statuses. (fixture-tested; local tests do not prove a live deployment)
- Radicle broker, Nix source-to-build, exact-revision status and tag triggers.
  (exercised live on one same-host topology and across two owned tailnet hosts;
  not WAN or production)
- Tangled spindle-compatible service with service-auth-verified knot pushes and
  a reviewed workflow subset. (fixture-tested; no real knot has called Kiln)

**Execution and builds**

- Versioned Lattice workflow dispatch, lifecycle, controls, logs and
  reconciliation. (exercised live with GitHub)
- Authenticated Iroh transport to a remote worker through a controller-local Unix
  proxy. (exercised live with GitHub; not WAN or relay availability)
- Nix job discovery through `nix-eval-jobs` with bounded stream admission, task
  mapping and GC-root handoff. (exercised locally)
- Exact local Nix derivation builds with rooted outputs, bounded logs and a
  read-only cache projection. (exercised locally; no network acquisition,
  substitution or flake-evaluation claim)
- Opt-in parallel Rust frontend evaluation: a pinned-nightly `-Zthreads`
  benchmark under independent per-job CPU and memory scopes. It retains cgroup
  CPU, memory peak, OOM, queue and wall observations, measures rustc without a
  configured compiler-cache wrapper, and analyzes only same-nightly matched
  cells. A retained `onix-kiln-core` matrix at two contention levels recommends
  frontend threads no higher than the per-job CPU quota: threads=4 raised
  completed checks per minute by 41% with every quota reserved, while threads=8
  in a four-CPU quota lost 11% against threads=4. Build mode now finds a
  renamed library by its package, and its repeat hashes matched. Default builds
  are unchanged. (#88; one crate, host, nightly and quota)

**Gates, evidence and interchange**

- Exact Cairn gate and Mantle build request admission, bounded execution, result
  admission and reconciliation. (fixture-tested)
- Unsigned in-toto/SLSA provenance, CDEvents, OpenTelemetry and recorded Valence
  evidence. (fixture-tested; provenance is unsigned and makes no SLSA
  build-level claim)

**Web workspace**

- Disposable loopback-only website exploration via
  `cargo run --locked -p kiln-web-service --example explore`: a linked index
  covers every console destination and run-detail tab with native local CI,
  work/planning, review, source, receipt and raw-history data. Execution and feed
  observations are explicitly simulated; no provider, source command or
  notification delivery is invoked. Restart resets the workspace.
  A separate `website-exploration` package/app and immutable-path user unit
  support operator-authorized loopback deployment without changing live CI.
  Optional registered-user HTTPS ingress makes the examples available on a
  separate tailnet-only origin, with strict Host/Origin translation and no
  changes to the original CI origin, Tailscale Serve routes or Funnel exposure.
  (deployed as a separate local user service; browser and packaged checks passed;
  not CI/provider execution or production qualification)
- A pinned Bombadil browser rail over the real exploration console, with native
  catalog/session admission, read-only route/filter/history/tab/viewport actions,
  semantic selection/readiness/layout and HTTP/JavaScript properties, retained
  traces and explicit owned-versus-attached lifecycle. Dedicated tool/runner apps
  and a `web-testing` shell reuse the existing pin without production crate
  dependencies. Bounded campaigns do not establish exhaustive coverage,
  CI/provider correctness or release authority.
  (exercised with retained native success and layout-violation traces)
- Stable exploration cases: named examples carry a `case_id`, a canonical
  recipe with a BLAKE3 revision, source revision, presentation and interaction
  requirements and resolved subjects. `GET /examples/cases/<case_id>` resolves
  them and returns 404 for an unknown case and 409 for a moved recipe or another
  workspace. Plain examples receive no case identity. The index has a keyboard
  case selector, `scripts/explore-capture.py` retains bounded captures and
  `scripts/explore-compare.mjs` reports incompatibilities without normalizing
  runtime subjects. (the Rats migration cohort remains blocked on #67 and #68)
- A contained topbar for long workspace/page breadcrumbs and pinned navigation.
  Search uses intrinsic width, outer tracks and account controls can shrink,
  and labels elide without removing their accessible names or hiding document
  overflow. (rebuilt-browser geometry and account/palette keyboard interactions
  verified at mobile, tablet and desktop widths)
- Single-tenant HTTPS workspace with logs, reruns, signed triggers, schedules and
  notifications. (exercised locally with TLS 1.3; not production deployment)
- Operator-approved untrusted pull-request runs held in a durable blocked state
  and confined to declared runners without secrets, plus a source browser.
  (exercised locally; not a hostile-code sandbox)
- A GET-only OpenAPI 3.0.3 contract, `kiln.http-api.v1`, for projects, runs,
  execution bindings and artifacts in `contracts/http-api/openapi.json`.
  Independent schema validation, error goldens and a bounded, no-retry lockstep
  client fixture keep authentication, authorization, transport and schema
  failures distinct against the real HTTPS service and a loopback TLS fault
  stub, with byte limits and connect, total and body-idle deadlines. A pinned
  Drift check compares the contract with a reviewed baseline: incompatible
  changes fail outside a version transition and unhandled comparisons always
  fail. A measured Progenitor client was rejected. (#73; tested locally against
  the served process and fixtures; no generated client or server migration)
- An OpenBubbles iMessage receiver is a documented no-go: rustpush's SSPL
  exception covers only OpenBubbles. A typed Nickel receiver contract over the
  unchanged `kiln.webhook-notification.v1` payload, with positive and negative
  fixtures, is kept for a separately licensed future receiver; nothing is built,
  bundled or operated.
- One Kiln identity stylesheet shared by the live console, the offline catalog
  and report and the hosted bridge page, with a check that holds its contrast
  floor and rejects colours declared outside it. The console shows a running
  count from the last live snapshot and feedback for requests in flight, and its
  tables stack by the width of their own region. The offline documents and the
  hosted page keep 44px controls at 960px and narrower. (node tests; reviewed in
  a browser over synthetic data, not a running service)
- A technical graphics standard for every web surface, after the NASA Graphics
  Standards Manual: paper and ink, square framed panels, a monospace label
  layer, ink primary actions and identity red reserved for the brand mark, the
  title rule and the request line. A check rejects red elsewhere, rounded
  corners and blurred shadows. The console numbers every destination from one
  register read by the rail, title block and command palette, adds a Guide to
  destinations, run states, terms and keys, names timing a run cannot have yet
  and shows byte quantities in binary units. (node tests; reviewed in a browser
  over synthetic data, not a running service)
- One Patchgate receipt presentation for the receipts page and a run's Reviews
  tab. All nine admitted decisions read in their own words with their meaning
  and Patchgate's next step. Each change shows its message, paths, decider,
  time and reason from the native report, labeled as recorded by Patchgate. The
  list leads with the newest decision, names each receipt by its first change,
  and filters by text and decision. `/api/reviews` adds each receipt's first
  change message and latest decision time. (node and Rust tests; reviewed in a
  browser over the real reviews API with synthetic receipts, not a running
  service)
- A full-width Workers target register with immediate local filters, readable
  project names and independent exact selection. Target detail puts admission
  capacity, heartbeat evidence, latest execution and assigned work before exact
  binding identities. Expired heartbeat reports stay distinct from current
  availability, and the Overview shares the same slot-state vocabulary. (Node
  and Rust tests; live and synthetic browser review; CA-verified local and
  tailnet deployment of the committed package, without worker execution)
- Opt-in inventory v2 separates agent definitions, worker instances, compute and
  shared resource pools, and model-serving services. Authenticated console and
  explicit-file JSON/HTML consumers share exact-work explanations, freshness,
  rollout/qualification and reservation-versus-usage semantics. Existing worker
  v1 continues to describe configured project admission slots, not machines.
- Inventory v2 producer collection (Kiln #60). `kiln-adapter-lattice` maps
  Lattice `lattice.worker-inventory.v3` profiles to definitions keyed by their
  definition identity and retained instances to workers, never idle or healthy.
  v1 and v2 documents are refused as unsupported; v2 revisions were
  per-attempt, because they framed the task prompt and base snapshot, and are
  never compared with v3's stabilized revisions. Declared task families and
  tools are known capability, unavailable capability is a known negative, and
  undeclared capability stays unknown. The Lattice #25 revision lifecycle maps
  onto the definition's revisions (desired, prepared, qualified, selected for
  new sessions, rollout generation) and each worker's revision pin, with
  `unmanaged`, owned `unknown` and `recorded` kept distinct. A definition is
  projected at its selected revision while workers pinned to older revisions
  stay visible; two selected revisions or any other contradictory lifecycle
  fact is refused. Task-family qualifications keep their Animus #18 reference,
  evidence class and provenance; the readiness explanation shows the needed
  family's current reference and never counts `fixture` evidence as live, and
  a blocked pin blocks its worker. Single-shot liveness maps to the worker
  lifecycle, and retained generation orders repeated observations. Lattice wait
  reasons keep their exact code, subject and owner on definitions and workers,
  and a managed revision fact Lattice leaves unknown keeps its slot owner.
  They appear as owned explanation blockers through a provider-neutral core
  `WaitReason`; the new core revision, qualification and pin fields are omitted
  when empty, so earlier snapshots keep their identity.
  The adapter also maps
  `lattice.compute-allocation-observation.v1` (Aspen/Molten #42 records) into
  allocation facts and authority-keyed pools: a shared deployment or pool is
  counted once, and capacity stays unknown. New `kiln-adapter-animus` maps
  `animus.serving-availability-snapshot.v1` services with distinct loading,
  ready and saturated states, request capacity only from serving-owner slots or
  provider request quotas, withheld endpoints and no fictitious hosted
  accelerator. Pure
  `compose_snapshot` joins sources by exact producer identities, never host
  names. `kiln-web-service inventory-collect` reads an explicit
  `kiln.inventory.collection-set.v1` manifest. Unpublished producer facts are
  listed as unknown. (fixture-tested with verbatim Lattice v3 `inspect`
  captures of the Lattice #25 integration tree, including a fixture-provenance
  Animus #18 attachment, and Animus fixtures at 4071c65e; no live provider)
- Exact artifact-preview admission and review actions, with native complete
  runtime-closure/NAR/signature verification and a separately hosted disposable
  TryNix Linux engine. Engine-only rehearsal never invents a canonical CI build;
  unsupported engines, moved candidates and partial evidence fail preview-only.
  Clean-cache and warm engine-only Linux CLI launches are exercised; qualification
  against a genuine current canonical CI artifact remains separate.
- Preview evidence comparison: with an owner-controlled observation directory,
  the disposable preview server retains bounded browser reports as
  server-composed, BLAKE3-identified records copied from the admitted
  descriptor, with finite expiry, a refusing count bound and expired-record
  cleanup. Review projects them read-only, marks only the current revision's
  selected-run record current and compares one baseline with that candidate
  through `/preview-comparison`, reporting every engine, platform, argument or
  executable difference. The review page shows both lanes side by side from a
  deep link; the baseline never launches and observations never change CI,
  review or merge state. (exercised in one local canonical browser cohort; a
  historical baseline and a moved candidate are unit-tested only)
- Opt-in structural source evidence on the preview comparison (`structural=1`):
  the review's structural function compares the two observations' exact
  revision commits. The result is a BLAKE3-identified record bound to the
  comparison, both commits, the rule versions, the bounds and a grammar
  identity checked against `Cargo.lock`. Incompatible subjects, unreadable
  sources and oversized records keep no files and name their reason. The review
  page shows the record below both lanes only when it is selected. Display only;
  no review-time claim. (#65; frozen fixture equal to structural mode at route
  level, fake-provider fallbacks, Node renderer and a disposable browser layout
  check; no live preview cohort)
- A checked inventory of the live console as the reference for its
  Rats/Meteorite port: every registered destination with its rollout cohort,
  page panel, renderer and supporting modules, stylesheets, API paths, route
  parameters, states and the Node tests that load its renderer, plus the shared
  shell components and their anchors. A Node test in the `live-console-ui` check
  fails on any omitted destination or drifted reference, and the guide records
  which pages still lack a Node test of their renderer. (#68; Node checks over
  the served sources; no route ported and no visual parity claimed)

**Change review**

- Patch sets, exact diffs with interdiff, stack intake, ownership and
  revision-bound approvals; advisory approvals never count. (exercised locally)
- Exact-candidate required-CI merge automation and a separate speculative merge
  queue. (exercised locally; no deployment, publication, lifecycle acceptance or
  release authority)
- An off-by-default synthetic review-queue wait fixture runs a frozen, typed
  Nickel load through the real queue in serial and speculative modes, under a
  synthetic clock with one CI slot. Its receipt gives each candidate's exact
  identities, observed arrival, enqueue, start, CI and terminal times, and one
  named outcome. Review-queue wait is the only measured series; merge-queue,
  ready-work admission and inference series are recorded as not measured.
  Admission, queue and budget defaults are unchanged. (#61; frozen-load
  integration test and core fixtures; no production or external merge-queue
  measurement)

**Observation**

- Bounded read-only projection with typed profiles, paging, redaction and
  `kiln.observe.view.v1` identities. (fixture-tested)
- HTTPS and iroh observation with Ed25519 or UCAN auth, and a local dashboard.
  (exercised locally; views carry no authority)

**Optional hosted composition**

- Optional `kiln-hosted` with accounts, repository connections, reviewed
  pipelines and invoice drafts. (exercised locally; not a qualified paid service)
- Firecracker, Cloud Hypervisor and QEMU profiles and workload JWTs. (exercised
  locally; namespace backend trusted jobs only; `hostile_isolation_claim:false`)
- Opt-in retained Cloud Hypervisor host-effect journaling with operation-bound
  pause/resume controls that distinguish requests from native observed state,
  paired local checkpoints, exact handoff/fencing receipts and bounded direct
  Iroh migration transport. Missing provider compatibility remains unsupported;
  no live or two-host migration qualification follows from local contracts.
- Pinned Lattice worker guest (`lattice-worker-guest`): the reviewed kernel, an
  immutable initramfs with Lattice `661499d2` and admitted workflows, Cloud
  Hypervisor 54.0 and a `kiln.lattice-worker-guest.v1` manifest. Pure host and
  guest attempt admission, per-attempt workspace, guest identity and effect
  authority, and a dispatch that is journalled once and never resent. One
  host ran two concurrent Lattice workflows inside guests through the retained
  shell, with cold boot, pause, paired snapshot, retirement, restore and
  continuation. (#51/#52; exercised locally on one KVM host with a test-only
  reservation; no placement ownership, fencing, cross-host or live-migration
  qualification)
- Real hosted/Nix artifact-readiness measurement with exact cohorts, separate
  elapsed/CPU observations, cold/warm controls, retained failures and independent
  matched-cell analysis. An insufficient baseline cannot recommend a speedup.
- A fleet lifecycle qualification harness drives the real account, finance and
  fleet APIs through the unchanged immutable-executable admission against a
  fake provider with one-shot faults and barriers. Hosted job-secret
  capabilities and workload tokens bind to the observed active fleet
  generation: delivery, token exchange and resource-server validation refuse
  stale, replaced, unknown or uncertain generations, and observed retirement
  revokes them in the same transaction. Eleven recovery and secret-generation
  scenarios and two detected controls passed. (#72; exercised locally; not
  provider, capacity or physical-placement qualification)

**Implementation work**

- Atomic GitHub/Cairn intake/supersession, fenced attempts, immutable requirements,
  actual admitted isolated Lattice/OMP dispatch, durable cancellation/recovery,
  independently captured Git candidates and canonical Engine verification using
  the pinned native Cairn authoritative completion evaluator.
- Durable branch/PR effects and independent integration/source/Cairn observations,
  with reconciliation-only crash recovery. Unsafe ordinary GitHub merge and
  unconditional source closure are removed; no unattended landing is qualified.
- Opt-in bounded queues, reservation-time fairness accounting, parent budgets,
  exact owner grants and conservative unknown-effect reconciliation. Physical
  deployment and worker-request resource dimensions remain separate.
- An explicitly registered host-local resource owner for fair admission: a typed
  Nickel registration (`config/fair-admission-host-owner.ncl`) declares one
  host's capacity, admitted repositories and work classes and a finite campaign
  budget, and `kiln-implementer host-owner-admit|observe|reserve|release` derive
  the policy, account, queue rows, fresh eligibility, the owner-committed
  reservation the dispatch gate checks and observed-only settlement through the
  existing ledger and owner port. It reports no remote, provider or serving
  capacity; replay never duplicates a reservation or resets a budget.
- Under a configured admission policy, `kiln-implementer claim` selects and
  binds the queue row through admission, so the queue claim matches the attempt.
- GitHub intake policies accept an explicit `selection` of issue numbers and/or
  exact labels; unselected open issues are observed as `unselected` and never
  admitted by webhook drain or polling.
- `kiln-implementer policy-check --policy FILE --enrollment FILE` admits a
  GitHub intake policy with exactly the types, policy validation, work
  construction and ledger admission `poll` applies, using a synthetic issue
  and an in-memory ledger; it opens no database, credential or transport.
  Webhook intake and polling share one `ImplementationIssuePolicy::validate`.
  The probe uses the first enrolled actor a polled issue can carry.
- `kiln-web-service implementation-verification-graph` installs the canonical
  CI graph constructor that was the `implementation_verification_profile`
  example, with unchanged behaviour.
- The Lattice implementation CLI transport (Kiln #45) consumes Lattice
  `db64cd65`'s coordinator contract. Prepared requests carry the reserved
  `dispatch_plan` (the campaign prepares twice, since the plan frames the
  native worker profile), and a missing or different plan is refused before
  transport intent. `run` receives exactly the one `env` variable the request's
  `omp.model_broker.credential` names, refused right after claim authorization
  and before plan reservation when missing, malformed, non-`env` or reserved;
  `recover` and `cancel` receive none. The first `run`'s transport intent
  records its Lattice program and state root (`TransportLaunch`) in the same
  transaction, under its own implementation-state kind `transport_launches`
  so older binaries re-saving `attempts` cannot erase it; it is never
  overwritten, and `recover` uses only that recorded
  launch; an intent without one, a missing recorded root or a changed program
  keeps the unknown outcome without invoking Lattice. Only a `run` refusal that is exactly
  Lattice's `lattice.implementation-attempt-refusal.v1` record for the attempt,
  with `dispatched:false` and reason `crossed_attempt`,
  `profile_revision_superseded`, `revision_not_admitted` or `revision_blocked`,
  ends it as an infrastructure failure with nothing to reconcile, and a bound
  recovery's exact `not_started_closed_by_recovery` record is recorded as no
  execution; `stale_fence`, every `recover` refusal and every contradictory
  record stay for reconciliation. `config/implementation-worker.ncl` and
  campaign binding admit only transport v2 (the direct worker runtime keeps its
  v1 branch), and the local fixture and runbook use request v3 with
  `network:false` and the brokered model route. Breaking for prepared requests
  without `dispatch_plan`. (workspace tests; composed local qualification with
  the real Lattice binary and a local HTTP 400 endpoint, no model call)
- Native-consumer qualification campaigns and frozen positive/negative controls,
  plus an opt-in onix-core deployment/guest composition. Actual model routes,
  isolation/grants, immutable executables, canonical writer access and approved
  remote effects remain explicit prerequisites; no deployment was performed.
- Bounded selected-Cairn campaign continuation over canonical work, native
  Lattice, verification and delivery owners, with exact integrated-successor
  source identities and retained per-unit obligations. Empty readiness and
  unknown candidate/policy/verification evidence cannot complete a campaign;
  pause/cancel closes new decisions and work, repository writers remain bounded,
  and independent eligible work can progress past a blocked member.
- Typed Cairn `cairn.selected-work.v1` intake (Kiln #83) from `cairn-core`
  `8f6cd0b373962ca8c370e00ef20dfbf0c398e0d0`: producer admission
  and mapping into provider-neutral core facts, immutable per-attempt snapshot
  contracts with refresh/replan transitions, one obligation projection for worker
  context, campaign inspection and the UI, obligation completion over canonical
  checks and authoritative completion, a restricted display that cannot be
  resubmitted, and Lattice `lattice.workflow-intent.v1` normalization with
  compiled-plan admission and proposal refusal. Under `typed_snapshot_v1`,
  pre-attempt member stages follow the admitted snapshot's native phase
  readiness, stable blockers, units and dependencies; an unavailable or
  incomplete snapshot never makes a member ready. The legacy reader stays
  authoritative only under the explicit `legacy_readiness_compatibility`
  intake. (fixture-tested; no live campaign)
- Selected campaigns claim and reattach through the canonical scoped admission
  transaction, bind actual parent-account ancestry and exact retained decision
  projections, and require the named canonical checks. Native source observation
  uses bounded immutable commit snapshots and explicit remote-target freshness;
  it does not inherit ambient credentials or qualify unsupported delivery goals.
- Selected campaigns resume through `kiln-web-service
  continue-implementation-campaign`, a one-shot step a supervisor invokes. Each
  step rereads canonical state and runs at most a configured number of bounded
  driver batches. It drives on a coalesced wake hint
  (`wake-implementation-campaign`, or an operator `continue` or decision
  answer), a moved campaign version, an operation-limit batch, a retry after
  backoff or the bounded rescan; otherwise it waits or stops. Wake demand,
  failures and backoff persist beside the campaign. Only driver failures count
  against the budget: a stale version rereads, and a canonical claim refusal
  (another winner or a busy repository writer, now typed `writer_busy`) waits
  for capacity. (#87; unit and scripted-owner fixtures; no live campaign or
  supervisor deployment)
- Operate selected campaigns from the console. With `serve --campaign-controls`,
  a Nickel-checked profile grants exact principals campaigns, commands, review
  roles and an optional expiry, and lists frozen campaign definitions with
  their delivery route. The Workers campaign card shows start previews (exact
  members, goals, checks, reviews, policies and the goals the route offers),
  admission controls and a decision inbox with answer forms. The canonical
  owner applies each command and records its request ID, actor and outcome in
  the campaign database in one transaction, so a retry after a lost response
  returns the recorded outcome instead of applying twice. Stale versions,
  changed definitions and changed decision scope return refreshable conflicts;
  missing, narrower or expired grants and the wrong review audience are
  refused and audited before the owner is reached. The campaign operator gains
  `pause`. (#86; unit, scripted-store, Node and browser checks over the
  synthetic exploration workspace; no live campaign)
- A full-width **Cairn changes** register (2.7) presents List and Board views
  over the same deterministic lane, blocker, decision, name and repository order.
  Search, lane, blocker and decision filters, selected change and expanded unit
  survive URL reloads; Previous/Next and Browse changes preserve the filtered
  register. Read-only index/detail `v2` projections join exact canonical work,
  attempts, verification, publication, lifecycle and campaign members; the
  console refuses `v1`. Lanes and unit spread summarize separately visible
  readiness, execution, evidence, integration and lifecycle facets, never
  acceptance authority. Detail leads with ranked blockers, owners and open
  decisions, then expandable units and historical/current attempt timelines,
  exact dependency/dependent graph including cycles and not-current work,
  campaign members and disclosed copyable identities. Only answer/issue actions
  listed by the canonical campaign inbox for this change's exact members appear
  inline; the shared command runner retains store-version, request-ID retry and
  conflict behavior. The disposable explorer has A→B, historical C and
  completion-only D without provider execution. (#84; synthetic local rehearsal,
  not a live campaign, worker or lifecycle completion claim)
- The Cairn change detail (`kiln.cairn-workspace-detail.v3`) gains an **Agents
  and revisions** section from the admitted resource inventory, never from
  producer documents: each definition's desired, prepared, qualified and
  selected revisions as `recorded`, `not_recorded`, `unknown` with its owner,
  `unmanaged` or `unpublished`; task-family qualification references with
  evidence class and provenance, where `fixture` never reads as live; declared
  capability; each worker's pinned revision against the selection, its pin and
  blocked state; and owner-attributed wait reasons. Workflow revisions are a
  separate slot, unpublished with owner `lattice#25`. Sessions, workspaces,
  execution and grant references, model-service bindings, full qualification
  records, preparation failures and other work's attempts are never copied. An
  unconfigured or refused inventory is named and never fails the detail; the
  console refuses `v2` details. (#84 for Lattice #24 and #25; projection tests
  over the Lattice v3 fixtures, route and Node UI tests; no live provider)
- The Cairn change detail (`kiln.cairn-workspace-detail.v4`) shows Lattice's
  workflow revisions in **Agents and revisions** instead of a fixed
  `unpublished` slot. Kiln reads `lattice.workflow-agent-revision-slots.v1`
  through the inventory mechanism: a `workflow_revisions` source in the
  collection manifest and profile, a schema-probed, byte-bounded strict
  adapter (`kiln_adapter_lattice::workflow_revision_slots`) and provider-neutral
  `WorkflowRevisionSlots` facts. Each workflow definition's desired, last usable
  and selected revision reads `recorded`, `not_recorded` or `unknown`, with its
  compiler, plan, graph and Cairn cohort identities and the worker revision it
  pairs with, compared with the inventory's projected worker revision; pins are
  counted per revision with status and blocked state. Session identities, names
  and guidance, context or source identities are never copied. A missing or
  refused slots document makes only the workflow source `unavailable` with its
  refusal code, or `not_configured`; the rest of the detail is unchanged. The
  console refuses `v3` details. (#84 for Lattice #25; verbatim captures from
  Lattice `7fb6dc47` with fixture ports, adapter, collection, projection, route
  and Node UI tests; no live provider)
- Adopt already implemented work for a freshly selected completion-only Cairn
  change instead of blocking it. Every required native unit needs current
  canonical work whose latest attempt is verified with exactly one passing
  authoritative Cairn completion check and integrated through the selected
  route for that exact work, attempt and fence; the integrated result must be
  contained in the current target with the change source unchanged since. The
  member then binds the attempt whose result contains the others at its
  integrated stage, so the existing lifecycle step can run without a claim or
  coding dispatch. Each missing or stale fact stays a named blocker. (#47; pure
  decision, ledger extraction and local Git fixtures; no live campaign)
- Opt-in ordered-stack publication of one verified attempt. The single verified
  pull request stays the default and is kept on hosts without stacked pull
  requests; Origin stacks are refused before any effect. The attempt's verified
  candidate is the top layer, and each lower layer is an ancestor with its own
  canonical CI verification under the same rules. A plan binds two to sixteen
  exact layers, their parents and a deterministic branch chain. The GitHub
  adapter probes stack support, checks every layer's tree and ancestry and reads
  every layer before the first push. It then publishes branches and pull
  requests from the bottom up and creates or reuses one exact stack; a re-run
  only reads. A layer may land only over ready or integrated layers, and
  read-only recovery admits only an exact bottom prefix of two-parent merges.
  Kiln never merges, rebases, retargets or unstacks. (#89; pure-core, local bare
  Git and loopback GitHub fixtures; not exercised on live GitHub, and not yet
  selectable from the operator CLI)
- Opt-in plan review before implementation admission. An operator-configured
  policy names repositories and reviewers; without it, eligibility, claims and
  the stored work ledger are unchanged. A plan is pinned by BLAKE3 to one exact
  work version, and each revision names the plan it replaces. Reviewers other
  than the author annotate a whole step or a text span with a comment, a
  deletion or a suggested replacement, resolve blocking feedback, and accept
  or request changes on the latest revision. Selected work stays blocked with
  `plan_not_accepted` until its exact version has an accepted plan; acceptance
  itself claims nothing. `kiln-implementer plan-review`, `plan-feedback` and
  `plan-review-receipt` apply commands, return quoted feedback with a revision
  comparison, and replay frozen synthetic cases (one changed, one prevented,
  one unchanged, two unavailable). (#93; pure-core, durable-store, CLI and
  Nickel-fixture tests; no captured agent plan or measured quality gain)

### Changed

- `poll` now refuses an invalid repository policy (for example
  `repository_id` 0) or base reference before any GitHub call, as webhook
  intake already did. It previously checked only the selection, so such a
  policy could admit polled work.

### Fixed

- Serve the GitHub DNS helper entry (`--internal-github-status-dns <ppid>`) in
  `kiln-implementer` and `kiln-hosted`, not only `kiln-adapter-github`, through
  one `run_internal_resolver_if_requested` call first in each `main`. GitHub
  clients re-run their own executable to resolve `api.github.com`, so
  `kiln-implementer` failed every `poll` and `publish` with an argument error.
  A process that never dispatched the entry now refuses to build the resolver
  (`github_status_dns_unserved`), and helper failures are named on stderr.
- Report the real cause of an implementer JSON input failure: malformed JSON
  and schema mismatches are distinguished, missing and duplicate field names
  are kept, every message gives the line and column, and messages that would
  quote input (an unknown member name or a value) are reduced to the mismatch
  kind and the type's expected names. Previously every
  failure read "<path> is not valid JSON", which hid a policy `base` without
  `contract_version` in the #49 pilot.
- Bring the accepted GitHub landing, source-status and publication
  requirements in line with the clients: unattended landing and issue closure
  are refused, only an exact external merge is recovered, and candidate upload
  before pull-request transport is a defined scenario. The stale merge and
  close scenarios left the specifications, and the active-development
  traceability profile no longer reports missing requirements. (specification
  and marker change; no behaviour change)
- Keep the random artifact-preview launch capability independent of its public
  descriptor identity, and bind both in the loopback path so the producer,
  review catalog and browser agree without admitting a substituted descriptor.
- Keep exact selected campaign IDs through workspace routing and filter-only
  refreshes, so campaign obligations remain visible rather than resetting.
- Accept exact-owner source-retirement observations after observed cleanup,
  while refusing active handoff commands against an already retired source.
- Validate raw independent-verifier check records against canonical provenance
  rather than the unrelated uploaded-report envelope. Persist unknown intents
  without inventing check evidence; successful records still require every
  exact retained check artifact and receipt binding.

- Unify local Git/GitHub and COB consumers on `git2 0.21.0`, fixing
  empty-remote publication's null-slice failure and migrating fallible string
  accessors without changing SHA-1 or the pinned native libgit2 version.
- Require the first native campaign observation to match its frozen profile
  and policy, and share canonical selector keys across owners, projections and
  exploration examples. (local fixtures; no live campaign claim)
- Recheck qualification programs, retained input documents and protocol
  templates before stage execution; changed inputs remain nonaccepting.
- Keep the native canonical-preview fixture's Nix source access, pure store
  dependencies, Jujutsu path selection and derivation-byte identity scoped to
  its exact owned store and source repository.

- Bind required typed checks to the same selected exact project/scope CI run
  used for review admission, rather than borrowing historical or unrelated
  passes. Require terminal, successful counting evidence and display the actual
  selected run. Monorepo typed requirements remain review-project scoped. (#64)
- Keep selected campaigns advancing independent eligible work while another
  member waits on a required review, required context, replan, delivery
  capability or endpoint evidence. Selection evaluates every member, keeps each
  member's hold, and surfaces a hold only when no member can act. Writer claims
  still wait for the single repository writer, and canonical admission still
  decides claims and budgets. The drive output and the campaign projection show
  the selection basis and every member's hold. (#85; unit and scripted-owner
  fixtures; no live campaign run)
- Open the synthetic exploration campaign from its catalog link. The link used a
  `campaign` parameter, but the Workers page restores `worker_campaign`, so the
  example opened without a campaign loaded. (browser-checked locally)
- Refuse a campaign decision answer once the member's current work, policy or
  candidate no longer matches the decision, with
  `campaign_decision_scope_changed`. The same scope rule already admitted the
  request; an answer for a superseded candidate could previously resolve its
  successor's review. (#86; core and SQLite store tests)
- Refuse a worker acknowledgement or dispatch reconciliation whose provider run
  is already recorded on another attempt, with `implementation_dispatch_crossed`.
  A replacement attempt could previously adopt its predecessor's run as its own
  execution. Fixtures now cover the rest of the #45 race matrix: late callbacks
  after cancellation, predecessor callbacks after a replacement and callbacks for
  attempts that were never admitted, restoring the ledger between callbacks; the
  implementation runbook maps every case to its fixture. (#45; pure ledger
  fixtures; no assembled provider race)
- Keep verified content-index updates pending across equal `put_exact` retries
  until payload synchronization, exact replacement and parent synchronization
  complete. Persistent create/write/sync/rename faults cannot become a successful
  retry acknowledgement; published mappings survive failed additions and exact
  readback survives recovery/reopen. Eight focused real-port tests include the
  ten-case empty/populated index failure matrix. No power-loss guarantee is
  inferred from successful filesystem synchronization.

### Packaging and installation

- Distribution is source plus the Nix flake. The flake declares apps for
  `kiln`, `kiln-web-service`, `kiln-hosted`, `kiln-dashboard`,
  `kiln-adapter-github`, `kiln-github-job`, `kiln-adapter-gitea`,
  `kiln-gitea-job`, `kiln-adapter-iroh`, `kiln-radicle-ci`, `kiln-implementer`,
  `kiln-lattice-dispatch`, `kiln-cairn-intake`, `kiln-patchgate-review`,
  `kiln-nix-cache`, `kiln-dashboard-refresh` and `kiln-dashboard-publish`.
- `kiln --help`, `kiln -h` and `kiln help` print the operator command's usage
  and exit 0; a missing or unsupported command is refused on standard error.
  (workspace tests)
- `kiln --version` and `kiln -V` print `kiln` and the package version. The
  other shipped programs have no version flag. (workspace tests)
- Enter the development shell with `nix develop path:$PWD`.
- No crates.io publication, binary release, container image or
  package-manager entry is prepared; 36 of the 46 workspace crates set
  `publish = false`.
- The core and command-line packages are named `onix-kiln-core` and
  `onix-kiln-cli`; the library `kiln_core` and the binary `kiln` keep their
  names. Unrelated crates named `kiln`, `kiln-core` and `kiln-cli` exist on
  crates.io, so `cargo install kiln-cli` does not install this project.

### Compatibility notes

- No compatibility policy is published for `0.1.0`. Commands, schemas and
  profiles may change before a later release.
- Schemas and profiles carry explicit version identifiers such as
  `kiln.observe.view.v1` and `kiln.packed-preserves.v1`.
- A consumer that pins Kiln by Git revision declares
  `kiln-core = { package = "onix-kiln-core", ... }` once its pin includes the
  package rename. Earlier revisions keep the name `kiln-core`.
- Retained evidence names `x86_64-linux`. The flake also declares
  `aarch64-linux`; its outputs evaluate but were not built or run.
- The workspace pins Rust 1.97.1 in `rust-toolchain.toml` and declares
  `rust-version = "1.97.1"`. That is the tested compiler, not a proven minimum.
- Protected implementation operations deny until a verified UCAN adapter or an
  explicit local-operator profile is selected. `prepare_publication` and
  `record_integration` take the host time used for currentness. A caller
  boolean, role label, provider capability, serialized receipt or retained
  session is not authority.

### Non-claims

- No production readiness, stability guarantee, SLA, uptime or support response
  commitment is claimed.
- Kiln is not a sandbox and claims no hostile-code confinement or hostile
  multi-tenant isolation.
- No hosted or paid Kiln service exists; hosted CI is planned and not available,
  and no price is approved.
- Kiln claims no GitHub Actions compatibility and is not a drop-in replacement
  for another CI product.
- No exactly-once delivery or execution is claimed; unknown outcomes stay
  explicit until reconciliation.
- Emitted provenance is unsigned and makes no SLSA level or supply-chain
  certification claim.
- Tangled, Gitea and Forgejo interoperability is fixture-tested only, not
  established live.
- A passing decision or a historical receipt does not validate the current
  revision or prove source trust, build correctness or release eligibility.
- Agent and authorship metadata in review is self-declared and unverified.
