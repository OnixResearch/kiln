## Why

The OnixResearch stack has build, workflow, runtime, evidence, and lifecycle
systems. It does not have one provider-neutral owner for continuous integration
meaning. Placing CI projects, evaluations, runs, status, and fairness inside
Aspen, Mantle, or Lattice would make a generic provider own product semantics.

Hydra provides useful CI vocabulary and operator behavior, but its monolithic
control-plane assumptions do not fit the stack. Radicle CI provides a useful
broker and adapter split, but its native runner is intentionally local,
Git-specific, and unsafe for untrusted code.

Kiln needs an independent domain core. The core must work without Aspen,
Mantle, Lattice, GitHub, Radicle, Git, Pijul, or Jujutsu. Optional adapters can then bind
those providers without changing CI meaning.

## What Changes

- Create a provider-neutral CI domain for projects, pipeline revisions,
  triggers, evaluations, runs, tasks, attempts, products, and metrics.
- Define one pure command/event transition model that emits typed effect
  requests without performing external effects.
- Define versioned, capability-declaring provider ports for source,
  collaboration, triggers, status, workflows, builds, storage, notifications,
  evidence, and gates.
- Split source snapshots from forge collaboration so Git, Pijul, Jujutsu,
  GitHub, and Radicle can compose without false common semantics.
- Add Jujutsu source-provider mappings that preserve bookmarks, change IDs,
  commit IDs, backend kind, operation observations, and divergent-change facts
  without treating mutable change IDs or revsets as immutable revisions.
- Add a durable event-journal, outbox, replay, recovery, and read-model
  contract without claiming exactly-once external effects.
- Add deterministic tenant admission based on configured shares, selected
  multi-resource aggregation, and recent normalized use while leaving worker
  placement to providers.
- Add exact cross-project candidate sets, dependency graphs, frozen source
  universes, speculative gate policy, deterministic invalidation, and merge
  preflight without granting merge authority to the core.
- Add optional Remote Execution API compatibility that preserves content,
  action, operation, cache, capability, log, provider, and Kiln identities.
- Add non-authoritative SLSA, in-toto, CDEvents, and OpenTelemetry projections
  through versioned evidence, event, and telemetry boundaries.
- Add replay compatibility gates and split source discovery, acquisition, and
  mutation into separate admitted operations.
- Add a standalone reference profile that proves Kiln can run without any
  OnixResearch runtime provider.
- Plan Radicle as the canonical source and collaboration host without adding a
  Radicle runtime dependency to the core, standalone builds, or source archives.
- Require typed hosting policy, explicit publication authority, governed signed
  refs, independent acquisition, non-authoritative mirrors, and recovery plans.
- Add Radicle CI broker protocol compatibility and a separate Radicle status
  projection boundary.
- Use typed Nickel for human-authored configuration, canonical packed
  Preserves for Kiln artifacts, and BLAKE3 for Kiln-owned content identities.
- Add positive and negative conformance fixtures for every provider contract
  and state transition.

## Impact

- **Repository**: establishes the initial Kiln Rust, Nix, Nickel, Cairn,
  documentation, fixture, and evidence layout.
- **Core**: adds a `no_std + alloc` deterministic domain and transition kernel
  with no provider implementation dependencies, including no Jujutsu library or
  executable requirement.
- **Shells**: add standalone persistence, API, CLI, scheduler, and adapter
  orchestration without moving decisions out of the core.
- **Adapters**: establish optional provider crates for Jujutsu source access,
  Radicle CI, Remote Execution API, provenance, event interchange, and telemetry
  compatibility.
- **Hosting**: plans Radicle governance and read-only acquisition without
  assigning an RID, signing refs, publishing source, or changing remote authority.
- **Compatibility**: no existing product behavior changes. Aspen, Mantle,
  Lattice, Valence, Cairn, GitHub, Radicle, Git, Pijul, and Jujutsu remain
  external providers or references.
- **Testing**: requires positive and negative core tests, gating and fairness
  invariants, adapter conformance, crash and replay-compatibility fixtures,
  protocol fixtures, dependency-boundary checks, and focused Nix validation.
