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

- Single-tenant HTTPS workspace with logs, reruns, signed triggers, schedules and
  notifications. (exercised locally with TLS 1.3; not production deployment)
- Operator-approved untrusted pull-request runs held in a durable blocked state
  and confined to declared runners without secrets, plus a source browser.
  (exercised locally; not a hostile-code sandbox)

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

**Implementation work**

- GitHub issue and Cairn intake with claims, reservations, candidate handoff,
  CI verification and guarded landing. (fixture-tested; no live deployment)

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
