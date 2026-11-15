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

**Sources**

- Bounded read-only snapshots from local Git, Pijul and Jujutsu (Git backend).
  (Pijul rail exercised locally; Git and Jujutsu fixture-tested)
- Exact Radicle signed-reference acquisition and admission through a pure source
  contract that carries no provider runtime. (fixture-tested)

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
- Exact artifact-preview admission and review actions, with native complete
  runtime-closure/NAR/signature verification and a separately hosted disposable
  TryNix Linux engine. Engine-only rehearsal never invents a canonical CI build;
  unsupported engines, moved candidates and partial evidence fail preview-only.
  Clean-cache and warm engine-only Linux CLI launches are exercised; qualification
  against a genuine current canonical CI artifact remains separate.

**Change review**

- Patch sets, exact diffs with interdiff, stack intake, ownership and
  revision-bound approvals; advisory approvals never count. (exercised locally)
- Exact-candidate required-CI merge automation and a separate speculative merge
  queue. (exercised locally; no deployment, publication, lifecycle acceptance or
  release authority)

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
- Opt-in retained Cloud Hypervisor host-effect journaling, paired local
  checkpoints, exact handoff/fencing receipts and bounded direct Iroh migration
  transport. Missing producer compatibility remains unsupported; no live or
  two-host migration qualification follows from local contracts.
- Real hosted/Nix artifact-readiness measurement with exact cohorts, separate
  elapsed/CPU observations, cold/warm controls, retained failures and independent
  matched-cell analysis. An insufficient baseline cannot recommend a speedup.

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
- Native-consumer qualification campaigns and frozen positive/negative controls,
  plus an opt-in onix-core deployment/guest composition. Actual model routes,
  isolation/grants, immutable executables, canonical writer access and approved
  remote effects remain explicit prerequisites; no deployment was performed.

### Fixed

- Bind required typed checks to the same selected exact project/scope CI run
  used for review admission, rather than borrowing historical or unrelated
  passes. Require terminal, successful counting evidence and display the actual
  selected run. Monorepo typed requirements remain review-project scoped. (#64)
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
