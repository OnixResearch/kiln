# Kiln

Kiln is a planned provider-neutral control plane for continuous integration. It owns CI meaning and delegates external effects to replaceable adapters.

## Status

Kiln is in the design stage. The first active Cairn change defines these areas:

- CI domain types
- provider contracts
- durable state transitions
- Radicle repository hosting
- initial Radicle CI compatibility

## Owned boundary

Kiln will own these functions:

- projects, pipeline revisions, triggers, and evaluations
- runs, tasks, attempts, products, and metrics
- deterministic CI state transitions
- typed effect requests
- stable CI identities and canonical event records
- tenant admission, resource-accounting inputs, and fairness policy
- provider capability admission and adapter conformance contracts
- CI status, cancellation, retry, and notification meaning

Kiln will not own these functions:

- source-control history or forge authority
- workflow execution or build realization
- worker placement
- artifact truth or release authority
- runtime transport or deployment
- success of an external effect

## Architecture

The pure core will use these transitions:

```text
state + command -> events + requested effects
state + event   -> new state
```

Imperative shells will persist events. They will interpret requested effects through versioned provider ports.

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

Radicle is the planned canonical source and collaboration host. Kiln is not published, and it has no RID or remote authority.

Repository hosting does not create a Kiln runtime dependency on Radicle. Exact source archives and declared read-only mirrors must support acquisition without a Radicle node.

Mirrors are projections. They cannot replace Radicle governance without an explicit policy change.

Publication will require these items:

- a license decision
- a checked immutable candidate
- explicit operator authority
- governed signed references
- independent acquisition evidence

## Configuration and identity

Human-authored configuration will use typed Nickel contracts.

Kiln will use BLAKE3 for its content identities, event fingerprints, and deterministic identifiers. Interoperability protocols retain their required algorithms and native identities.

## Claim boundary

A passing Kiln decision proves only the checked relation between these values:

- admitted CI state
- one command
- emitted events
- requested effects

The decision does not prove that a provider performed an effect. It also does not prove source trust, build correctness, or release eligibility.

## Lifecycle

Accepted requirements will be in `cairn/specs/`. Active implementation changes will be in `cairn/changes/`.

## References

- [NixOS Hydra](https://github.com/NixOS/hydra) — CI vocabulary, evaluation model, fair-share scheduling, and operator surfaces.
- [Radicle CI](https://radicle-ci.liw.fi/) — node-local event filtering, CI-engine adapters, and decentralized operation.
- [Radicle CI broker architecture](https://radicle-ci-broker.liw.fi/architecture.html) — event normalization and adapter process boundaries.
- [Radicle native CI](https://radicle.network/nodes/radicle.liw.fi/rad%3Az3qg5TKmN83afz2fj9z3fQjU8vaYE) — compatibility protocol and trusted local runner reference.
- [Cairn](../cairn) — lifecycle, Radicle COB profiles, typed conflict handling, preview-first mutation, and interoperability evidence reference. Kiln retains CI identity and state authority.
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
