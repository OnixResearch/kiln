# Kiln

Kiln is a provider-neutral control plane for continuous integration. It owns CI meaning and delegates external effects to replaceable adapters.

**Exact-revision CI with replayable state and replaceable providers.** A pure
core turns each command into events and requested effects. Adapters for Git,
Jujutsu, Pijul, Radicle, GitHub, Gitea/Forgejo, Lattice and Nix perform those
effects. Every result binds to an exact revision, unknown outcomes stay
explicit, and recorded state can be replayed.

| Item | State |
| --- | --- |
| Version | `0.1.0`; no tagged release yet |
| Status | Self-hosted and under active development. No production or hostile-isolation qualification is claimed |
| Install | From source with the Nix flake; exercised on `x86_64-linux` |
| License | Mixed. Kiln-owned packages default to `MIT OR Apache-2.0`; see the [license map](LICENSES.md) |
| Source | Canonical on Radicle at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE`, with read-only HTTPS and GitHub projections |

[Quick start](#quick-start) · [What runs today](#what-runs-today) ·
[Documentation](docs/README.md) · [Detailed status](docs/status.md) ·
[Claim boundary](#claim-boundary)

## Why Kiln

- **CI meaning outlives providers.** Projects, runs, attempts, gates and
  status belong to Kiln. Source hosts, forges, executors and build systems sit
  behind versioned ports.
- **Results name their exact source.** Every gate result binds to one exact
  candidate identity. A moved target or failed dependency needs a new
  candidate and evaluation.
- **Unknown is not failure.** A transport failure after a provider accepted
  work produces `Unknown`. Reconciliation or operator policy resolves it.
- **History is decisions, not only logs.** Shells persist canonical events
  with BLAKE3 identities. Replay folds them back into state without returning
  provider effects.
- **Claims stay bounded.** Each surface states what a passing check does not
  prove.

## How it works

The pure core uses these transitions:

```text
state + command -> events + requested effects
state + event   -> new state
```

Imperative shells persist events. They interpret requested effects through versioned provider ports.

| Port | Possible providers |
|---|---|
| Source snapshots | Git, Pijul, Jujutsu |
| Collaboration and review | GitHub, Radicle |
| Triggers and status | GitHub, Gitea/Forgejo, Radicle, local hooks |
| Workflow execution | Lattice, a standalone runner |
| Build execution | Mantle, a standalone executor |
| Runtime and transport | Aspen/Molten, a local service shell |
| Evidence export | Valence |
| Release gates | Cairn |

Core crates must not import provider implementations. Provider identities remain explicit facts and never replace Kiln identities.

Read [the architecture](docs/architecture.md) for crate boundaries and
[provider authoring](docs/provider-authoring.md) for the port rules.

## Quick start

Acquire the read-only HTTPS projection without a Radicle node:

```console
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
  jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
cd kiln
```

This selects Git smart-HTTP protocol v0 for the Garden endpoint. Default negotiation returned HTTP 500 in the fresh-client check; the command above acquired the published source without credentials. These settings apply only to this clone, not global Git configuration.

The read-only GitHub projection needs only Git:

```console
git clone https://github.com/OnixResearch/kiln.git
cd kiln
```

Select and verify the intended exact revision before building; `main` is a moving reference.

```console
nix develop path:$PWD
cargo test --locked --workspace
cargo run --locked -p onix-kiln-cli -- demo
```

The first build compiles the workspace from source; no binary is published.
Kiln is not published to crates.io. Its command-line package is
`onix-kiln-cli` and its core package is `onix-kiln-core`; the unprefixed
`kiln`, `kiln-cli` and `kiln-core` crates there belong to unrelated projects.

`demo` commits one command to an in-memory journal and prints the resulting
state. It contacts no provider:

```json
{"revision":1,"runs":{"demo-run":{"active_attempt":"demo-attempt","outcome":"Pending"}},"attempts":{"demo-attempt":{"run_id":"demo-run","provider_profile":"standalone-build-v1","outcome":"Pending","external_reference":null,"products":[],"metrics":[]}},"holds":[],"seen_commands":["demo-start"]}
```

Render the local dashboard over synthetic sample data, then open the printed
`dashboard.html` in a browser:

```console
cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview
```

The sample is developer rehearsal, not evidence of real CI execution. The
[offline preview workflow](docs/observation-dashboard.md#developer-preview)
also supports source rebuilds with browser reload.

The full local release gate list, including `cairn release-readiness`, is in
[the operator guide](docs/operator.md#local-reference-run).

The committed, revision-verified imports cover direct and transitive workspace Cargo producer dependencies and Cairn/Mantle fixture inputs. Normal workspace builds require neither sibling checkouts nor operator Git credentials. See [build source acquisition](docs/reference-pins.md#build-source-acquisition) for exact pins and drift-refusing imports. Standalone runtime products retain their separately reviewed source cohorts and acquisition boundaries.

The [fresh standalone contributor check](.cairn/archive/1970-01-01-prepare-public-release/evidence/verification.md#current-standalone-contributor-recheck) records the exact public revision, acquisition environment, test/demo results, and bounded source-hygiene review.

To connect real sources, start with the [GitHub CI adapter](docs/operator.md#github-ci-adapter),
the [Gitea and Forgejo adapter](docs/gitea-ci.md), the
[Radicle source-to-build composition](docs/operator.md#radicle-source-to-build-composition)
or the [single-tenant web service](docs/operator.md#single-tenant-live-web-service).

## What runs today

Evidence words: **exercised live** means a retained receipt records a real
remote exercise for one recorded cohort; **exercised locally** means real
tools on one host; **fixture-tested** means tests and fixtures only. None is
a production claim, and no receipt validates later source.

| Area | What exists | Evidence | Start here |
| --- | --- | --- | --- |
| Deterministic core | Commands, events, requested effects and replay in a pure `no_std + alloc` crate; packed Preserves and BLAKE3 identities | Workspace tests | [Architecture](docs/architecture.md) |
| Sources | Bounded read-only snapshots from local Git, Pijul and Jujutsu repositories; exact Radicle signed-reference acquisition | Fixture-tested, with native Pijul, Jujutsu and Radicle tool rails | [Git](docs/git-source.md), [Pijul](docs/pijul-source-adapter.md), [Radicle](docs/radicle-source-shell.md) |
| GitHub CI | Authenticated push and pull-request intake, polling recovery, exact-source jobs through reviewed Lattice workflows, commit statuses | Exercised live for one recorded cohort | [Operator guide](docs/operator.md#github-ci-adapter) |
| Gitea and Forgejo CI | Signed webhooks, exact-source jobs, durable replay, commit statuses | Fixture-tested | [Procedure](docs/gitea-ci.md) |
| Radicle CI | Broker compatibility, source-to-build with Nix, exact-revision status, patch reviews, tag-triggered CI | Exercised live on one host and across two owned hosts on a private tailnet | [Operator guide](docs/operator.md#radicle-source-to-build-composition) |
| Tangled | Spindle-compatible service | Fixture-tested; no live knot has called it | [Integration notes](docs/tangled-integration.md) |
| Workflow execution | Lattice dispatch, lifecycle, logs and reconciliation; authenticated Iroh transport to a remote worker | Exercised live with the GitHub cohort | [Lattice](docs/lattice-workflow.md), [Iroh](docs/operator.md#iroh-remote-workflow-execution) |
| Nix | `nix-eval-jobs` job discovery; exact local derivation builds with rooted outputs and a read-only cache projection | Exercised locally | [Discovery](docs/nix-eval-jobs-adapter.md), [Build](docs/nix-build-adapter.md) |
| Gates and evidence | Cairn gates, Mantle builds, unsigned in-toto/SLSA statements, CDEvents, OpenTelemetry, Valence evidence | Fixture-tested | [Integration contracts](docs/integrations.md) |
| Web workspace | Single-tenant HTTPS service: run search, logs, reruns, signed triggers, schedules, notifications, operator-approved untrusted pull-request runs | Exercised locally | [Operator guide](docs/operator.md#single-tenant-live-web-service) |
| Change review | Patch sets, exact diffs with interdiff, Jujutsu change-ID and stack intake, revision-bound approvals, exact-candidate required-CI merge automation | Exercised locally | [Operator guide](docs/operator.md#optional-interactive-change-review) |
| Observation | Bounded read-only views over HTTPS or iroh; local terminal and HTML dashboard | Fixture-tested and exercised locally | [Projection](docs/observation-projection.md), [Dashboard](docs/observation-dashboard.md) |
| Hosted composition (optional) | Scoped accounts, GitHub repository connections, reviewed pipelines, spend admission, per-job microVM execution profiles | Exercised locally; not a qualified service | [Operator procedure](docs/hosted-ci.md) |
| Implementation work | GitHub issue and Cairn intake, claims, candidate verification, pull-request publication, guarded landing | Fixture-tested | [Runbook](docs/implementation-runbook.md) |

[Detailed status](docs/status.md) keeps the complete ledger: every implemented
area, each integration procedure with its retained receipt, and the
publication record.

## Hosted CI direction

The commercial plan keeps Kiln free/open source and proposes paid hosted CI
connected to customers' existing remote repositories, plus separately enabled
inference. It does not propose source hosting, repository migration, or a software
license/seat paywall. This is planning, not an available paid service. See the
[hosted compute launch plan](docs/commercial-launch.md) for remote-source onboarding,
accounting/payment architecture, company prerequisites and launch gates.

[Design-partner discovery](docs/gtm/design-partners.md) for the proposed pilot
is preparation for that plan's first gate, not an offer.

## Claim boundary

A passing Kiln decision proves only the checked relation between these values:

- admitted CI state
- one command
- emitted events
- requested effects

The decision does not prove that a provider performed an effect. It also does not prove source trust, build correctness, or release eligibility.

Kiln is not a sandbox, and local qualification is not production
qualification. See [security and non-claims](docs/security-and-non-claims.md).

## Owned boundary

Kiln owns these functions:

- projects, pipeline revisions, triggers, and evaluations
- runs, tasks, attempts, products, and metrics
- deterministic CI state transitions
- typed effect requests
- stable CI identities and canonical event records
- tenant admission, resource-accounting inputs, and fairness policy
- provider capability admission and adapter conformance contracts
- CI status, cancellation, retry, and notification meaning

Kiln does not own these functions:

- source-control history or forge authority
- workflow execution or build realization
- worker placement
- artifact truth or release authority
- runtime transport or deployment
- success of an external effect

## Licensing

Kiln-owned packages generally declare `MIT OR Apache-2.0`, except `kiln-adapter-pijul` (`GPL-2.0-only`). Linked AGPL/MPL dependencies and imported components retain their own terms; combined artifacts are not necessarily permissive-only. See the [license map and third-party notices](LICENSES.md). Owner-specific reuse grants do not extend to public recipients.

## Repository hosting

Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE`. Use its signed Radicle refs for current branch identities. Historical publication receipts do not validate a later branch tip or changed local source; the [publication record](docs/status.md#publication-record) keeps each receipt with its boundary, and `docs/hosting-and-recovery.md` states candidate and acquisition boundaries.

Radicle is the canonical source and collaboration host. The operator-authorized publication receipt is `evidence/publication-receipt.ncl`.

Repository hosting does not create a Kiln runtime dependency on Radicle. Exact source archives and declared read-only mirrors must support acquisition without a Radicle node.

Mirrors are projections. They cannot replace Radicle governance without an explicit policy change.

Publication records these items:

- the package-specific license decision, including the `MIT OR Apache-2.0` workspace default and the GPL/AGPL integrations described in `LICENSES.md`;
- the exact immutable candidate and archive BLAKE3 identity;
- explicit operator authority;
- the delegate, threshold, and parent-feature signed references;
- native and read-only HTTPS acquisition evidence.

## Configuration and identity

Human-authored configuration uses typed Nickel contracts.

Kiln uses BLAKE3 for its content identities, event fingerprints, and deterministic identifiers. Interoperability protocols retain their required algorithms and native identities.

## Contributing, security and support

- [Contributing](CONTRIBUTING.md): where work happens, checks, lifecycle and
  licensing of contributions.
- [Security policy](SECURITY.md): private vulnerability reporting and the
  security-relevant surface.
- [Code of conduct](CODE_OF_CONDUCT.md) and [changelog](CHANGELOG.md).
- Support: use the issue tracker on the
  [GitHub projection](https://github.com/OnixResearch/kiln/issues). No
  response time or support commitment is offered.
- [Launch kit](docs/gtm/README.md): positioning, release notes and the launch
  checklist.

## Lifecycle

Accepted requirements are in `.cairn/specs/`. Active implementation changes are in `.cairn/changes/`.

## References

- [NixOS Hydra](https://github.com/NixOS/hydra) — CI vocabulary, evaluation model, fair-share scheduling, and operator surfaces.
- [Radicle CI](https://radicle-ci.liw.fi/) — node-local event filtering, CI-engine adapters, and decentralized operation.
- [Radicle CI broker architecture](https://radicle-ci-broker.liw.fi/architecture.html) — event normalization and adapter process boundaries.
- [Radicle native CI](https://radicle.network/nodes/radicle.liw.fi/rad%3Az3qg5TKmN83afz2fj9z3fQjU8vaYE) — compatibility protocol and trusted local runner reference.
- [Radicle Heartwood](https://github.com/radicle-dev/heartwood) — collaborative-object storage and signing reference consumed through pinned crates.
- [SecretSpec](https://secretspec.dev/) — declared secret-provider interface consumed through the pinned `0.19.0` CLI.
- [Cairn](https://github.com/OnixResearch/cairn) — lifecycle and gate-interchange producer at revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f`. Kiln retains CI identity, process policy, gate composition, and state authority.
- [Mantle](https://github.com/OnixResearch/mantle) — build-interchange producer at revision `d75158b00b080535f2ffea12430665ca3d57ea82`. Kiln retains candidate truth, required-product meaning, CI state, retry policy, and release authority.
- [Bounded Exec](https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git) — bounded process mechanism at revision `29dac88ecded94457572db3fdfaaaab95fa91525`. Kiln retains executable authorization, output meaning, evidence, and retry policy.
- [Choregraph](https://seed.radicle.garden/zL2ncTUeASVYwcoGkEXv9JKgGbAF.git) — bounded execution-graph and pure transition reference.
- [Zuul](https://zuul-ci.org/) — exact candidate sets, speculative gates, dependency queues, and frozen repository state.
- [Concourse](https://concourse-ci.org/) — versioned resource discovery, pinning, acquisition, and mutation separation.
- [Bazel Remote Execution API](https://github.com/bazelbuild/remote-apis) — content, action, operation, cache, capability, and log-stream boundaries.
- [SLSA](https://slsa.dev/) and [in-toto](https://in-toto.io/) — source and build provenance vocabulary.
- [The Update Framework](https://theupdateframework.io/) — threshold, delegation, rollback, freeze, mix-and-match, and mirror threat models.
- [Kueue](https://kueue.sigs.k8s.io/) and [Dominant Resource Fairness](https://www.usenix.org/conference/nsdi11/dominant-resource-fairness-fair-allocation-multiple-resource-types) — admission fairness references.
- [Temporal](https://temporal.io/) — deterministic replay compatibility and command-history audit patterns.
- [CDEvents](https://cdevents.dev/) and [OpenTelemetry CI/CD](https://opentelemetry.io/docs/specs/semconv/cicd/) — non-authoritative event and observability projections.
- [Buildbot](https://buildbot.net/) — change-source, scheduler, build-request, worker, and reporter separation.
- [Pueue](https://github.com/Nukesor/pueue) — operator status, logs, waiting, dependency visibility, local capacity groups, scoped controls, and crash-recovery test patterns. Kiln does not use its scheduler, protocol, runtime, types, or provider model.
- [Jujutsu](https://github.com/jj-vcs/jj) — source-provider semantics for bookmarks, stable change IDs, exact commit IDs, divergent changes, operation observations, and Git-backed interoperability. Kiln does not treat Jujutsu as a forge or collaboration provider.
- [Lattice](https://github.com/OnixResearch/lattice) — producer-owned workflow contract and bounded Unix transport at revision `70496e67c7fd4a8b05914161a8e09de2759bebc8`. Kiln retains CI state and outcome authority.
- [nix-eval-jobs](https://github.com/NixOS/nix-eval-jobs) — optional stock Nix job-discovery provider at revision `a0cd02231c58974a6b5aaa3712069b071047162e`. Kiln retains source admission, task mapping, CI state, build dispatch, and release authority.
- [Nix](https://nixos.org/) — exact local derivation build provider using Nix `2.34.8` and a pinned executable identity. Kiln retains build-plan, product, outcome, evidence, and release authority.
- [Pijul](https://nest.pijul.com/pijul/pijul) — native channel states, content-addressed changes, and bounded local snapshot acquisition. Kiln does not claim source trust, history completeness, or collaboration authority.
- [Aspen/Molten](https://github.com/brittonr/aspen) — workload-neutral runtime, native system-extension host, durability, content, transport, scheduling, and bounded-execution contracts at revision `22f8ded26ca1907c29948e08b53f35df23080733`. Kiln retains all CI meaning, provider selection, retry, outcome, and release authority.
