# Kiln

Kiln is a provider-neutral control plane for continuous integration. It owns CI meaning and delegates external effects to replaceable adapters.

## Status

Kiln has an initial functional core and standalone reference shell. The implementation includes these areas:

- deterministic CI commands, events, effects, and replay;
- versioned provider capability admission;
- bounded read-only source snapshots from local Git and Pijul repositories,
  including a pinned native Pijul source-to-CI E2E rail;
- versioned Lattice workflow dispatch, lifecycle, controls, logs, and reconciliation;
- authenticated Iroh workflow transport through a private local Unix proxy and worker-local Lattice endpoint;
- exact Cairn gate request, process, result, and reconciliation admission;
- exact Mantle build request, bounded process, product, log, and reconciliation admission;
- exact stock and separately selected Onix Nix job discovery, bounded stream admission, task mapping, and GC-root handoff;
- source, gating, fairness, task-operation, hosting, remote, interchange, and COB decisions;
- packed Preserves and BLAKE3 canonical identity;
- bounded, deterministic, read-only observation projection over admitted state with typed profiles, paging and window admission, redaction, and `kiln.observe.view.v1` identities;
- a bounded HTTPS observation shell with canonical JSON views, a bounded SSE event leg, Ed25519 or UCAN holder-invocation authentication, TLS, and no control surface;
- a bounded iroh observation shell for allowlisted peers with BLAKE3-verified payloads and maintained HTTP/iroh view equivalence;
- an exact Aspen/Molten runtime profile, atomic event-store mapping, bounded content and callback contracts, and fail-closed ingress;
- a standalone `kiln-aspen-host` composition shell that runs the real Aspen native service and concrete Lattice effect adapter;
- exact-cohort host qualification receipt admission, retained-artifact identity checks, and stronger-claim rejection;
- a standalone `kiln-radicle-nix-provider` shell for bounded checks and durable reports under Lattice;
- an exact local Nix derivation build provider with discovery-task composition, durable state, rooted outputs, bounded logs, and reconciliation;
- an isolated signed-operator OnixOS pilot joining the maintained Aspen/Lattice runtime to source-bound discovery, rooted application builds, canonical journal replay and UCAN-authenticated observation;
- Radicle broker compatibility, exact signed-reference source acquisition, bounded CI status publication, and declared single-writer patch reviews;
- authenticated GitHub push/PR intake, exact HTTPS source jobs through reviewed Lattice workflows, durable lifecycle recovery, and bounded commit-status publication;
- typed Nickel configuration and Nix checks.

Kiln is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE`. Use its signed Radicle refs for current branch identities. The historical operator-authorized publication receipt covers commit `36984e367eafd44c22d2b4f4f47121deb51eece3`; it does not validate a later branch tip or changed local source. The later source-only cutover to `f748acd0cc239f48bf3565ccb89d46ce88cbee2f` and its candidate-bound checks, including the optional runtime attributes that were not exercised, are recorded in [`.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/`](.cairn/archive/1970-01-01-prepare-public-release/evidence/public-cutover-2026-10-18/publication.json). See `docs/hosting-and-recovery.md` for candidate and acquisition boundaries.

The subsequent operator-authorized main cutover to
`6c6e4f220fe0234ce607085fa622b580fd1cd016` has its own
[source/acquisition receipt](.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/main-cutover/receipt.json).
The pilot's frozen-cohort qualification is independent of later source
publication; publishing these sources does not qualify changed executables.

## Quick start

Acquire the read-only HTTPS projection without a Radicle node:

```console
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
  jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
cd kiln
```

This selects Git smart-HTTP protocol v0 for the Garden endpoint. Default negotiation returned HTTP 500 in the fresh-client check; the command above acquired the published source without credentials. These settings apply only to this clone, not global Git configuration.

Select and verify the intended exact revision before building; `main` is a moving reference.

```console
nix develop path:$PWD
cargo test --locked --workspace
cargo run --locked -p kiln-cli -- demo
```

The full local release gate list, including `cairn release-readiness`, is in
[the operator guide](docs/operator.md#local-reference-run).

The committed, revision-verified imports cover direct and transitive workspace Cargo producer dependencies and Cairn/Mantle fixture inputs. Normal workspace builds require neither sibling checkouts nor operator Git credentials. See [build source acquisition](docs/reference-pins.md#build-source-acquisition) for exact pins and drift-refusing imports. Standalone runtime products retain their separately reviewed source cohorts and acquisition boundaries.

The [fresh standalone contributor check](.cairn/archive/1970-01-01-prepare-public-release/evidence/verification.md#current-standalone-contributor-recheck) records the exact public revision, acquisition environment, test/demo results, and bounded source-hygiene review.

Onix Nickel/WASM discovery is an explicit optional profile, not the stock default.
Provision its separate pinned cohort and run the real discovery rail using the
[Onix procedure](docs/nix-eval-jobs-adapter.md#explicit-optional-onix-profile). The
[immutable fixture receipt](runtime/onix-eval-cohort/jobset.source.json) records
the imported OnixOS source. Core and stock-only builds do not acquire the Onix
evaluator or plugin; passing discovery does not prove a build or deployment.

The [isolated maintained-cohort OnixOS pilot](docs/operator.md#isolated-maintained-cohort-onixos-pilot)
exercises a real home-activation application launcher, duplicate delivery,
host restart/reconciliation and genuine evaluation/build failures. Its exact
input and runtime receipts bound the claim: generated activation/home-file
dependencies are prepared separately, and the pilot neither executes activation
nor establishes autonomous forge delivery, production cutover or release authority.
The [cohort-bound qualification receipt](.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/verification.json)
retains the actual outcomes, source/input identities, exercised gates and native
lifecycle closeout. It does not qualify another source or executable cohort.

Read `docs/architecture.md` for crate boundaries. Read `docs/observation-projection.md` for the bounded read-only observation model. Read `docs/http-observation.md` for the HTTPS view and SSE transport contract. Read `docs/iroh-observation.md` for the peer-verified iroh transport and equivalence contract. Read `docs/nix-build-adapter.md` for the exact local Nix derivation build provider. Read `runtime/kiln-aspen-host/README.md` for the deployable Aspen host composition and its bounded claims. Read `docs/operator.md` for operator behavior. Read `docs/radicle-source-contract.md` for exact signed-reference and snapshot evidence. Read `docs/radicle-source-shell.md` for bounded local acquisition. Read `docs/radicle-workflow-shell.md` for source-bound Radicle trigger dispatch through Lattice. Read `docs/radicle-nix-provider.md` for bounded hosted Nix checks and durable reports. Read `docs/radicle-status.md` for SecretSpec-resolved, single-writer CI status publication. Read `docs/radicle-mutation.md` for declared patch review mutation. Read `docs/cairn-gate.md` for exact lifecycle-gate evaluation. Read `docs/nix-eval-jobs-adapter.md` for bounded stock Nix job discovery. Read `docs/pijul-source-adapter.md` for the Pijul compatibility cohort and claim boundary. Read `docs/aspen-runtime-backend.md` for Aspen setup, ownership, recovery, status, and rollout gates.

The status and patch-review shells use the separately pinned v0.2.0 shared Git COB store imported into this repository, not a sibling checkout. See `docs/radicle-status.md` for its immutable source digest and drift-refusing import procedure.

For GitHub CI, use the typed builders `config/github-ci.ncl` and
`config/github-job.ncl` with real operator-supplied repository IDs, SecretSpec
references, executable/profile inputs, and exact reviewed workflow identities.
The default package installs `kiln-adapter-github` and `kiln-github-job`, also
available as Nix apps. Follow the [end-to-end operator setup](docs/operator.md#github-ci-adapter)
for HTTPS webhook ingress, Lattice `route_exact`/`shell_environment` routing,
job JSON byte pinning, permissions, status, and recovery. `fixtures/github/`
contains isolated synthetic conformance inputs, not production configuration.
Kiln maintainers own this integration's repeatability rail; operators retain
deployment, credential, worker isolation, and repository authority. Local checks
do not prove live deployment, remote status writes, source trust, or confinement.
Recorded local outcomes, executable identities, and scoped producer-gate results are in
the [scoped GitHub CI evidence](evidence/github-ci-local-runtime.ncl).

For remote workflow execution over Iroh, use `kiln-adapter-iroh` with the existing
consumer's Lattice socket setting. Follow [peer enrollment, private keys, profiles,
startup, and recovery](docs/operator.md#iroh-remote-workflow-execution). The
[change-local evidence](.cairn/archive/1970-01-01-add-iroh-workflow-transport/evidence/verification.json)
records actual local execution and exact recovery after both bridges restart,
not WAN/relay availability or deployment authority.

The [combined live CI proof](.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json)
joins genuine GitHub deliveries to exact-source compile/test jobs on a separate
Iroh worker, canonical passing/failing results, and matching Published GitHub
statuses. It also records controller/bridge restart recovery of the original
still-running execution and genuine redelivery suppression. Follow the
[bounded live exercise](docs/operator.md#bounded-combined-live-ci-exercise) for
the recorded cohort, repetition, credential retirement, and claim limits.

## Licensing

Kiln-owned packages generally declare `MIT OR Apache-2.0`, except `kiln-adapter-pijul` (`GPL-2.0-only`). Linked AGPL/MPL dependencies and imported components retain their own terms; combined artifacts are not necessarily permissive-only. See the [license map and third-party notices](LICENSES.md). Owner-specific reuse grants do not extend to public recipients.

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

## Architecture

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
| Triggers and status | GitHub, Radicle, local hooks |
| Workflow execution | Lattice, a standalone runner |
| Build execution | Mantle, a standalone executor |
| Runtime and transport | Aspen/Molten, a local service shell |
| Evidence export | Valence |
| Release gates | Cairn |

Core crates must not import provider implementations. Provider identities remain explicit facts and never replace Kiln identities.

## Repository hosting

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

## Claim boundary

A passing Kiln decision proves only the checked relation between these values:

- admitted CI state
- one command
- emitted events
- requested effects

The decision does not prove that a provider performed an effect. It also does not prove source trust, build correctness, or release eligibility.

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
- [Choregraph](../choregraph) — bounded execution-graph and pure transition reference.
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
