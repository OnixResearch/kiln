# Proposal: Add the Aspen runtime backend

## Why

Kiln already declares `aspen-runtime-v1`, but that profile only records `service_hosting` as a capability. No adapter, extension contract, or composition root uses Aspen.

The current Radicle shell selects Lattice and SQLite directly. This path cannot place Kiln's durable service state, trigger intake, and provider-effect orchestration on Aspen.

Aspen can supply typed transport, durable state, content storage, scheduling, supervision, and bounded execution. Kiln must still own CI commands, events, candidates, attempts, policy, retries, and outcomes.

## What Changes

- Pin one reviewed Aspen contract and source cohort through immutable repository coordinates.
- Add a Kiln-owned Aspen system-extension contract with canonical request, callback, state, effect, and status schemas.
- Add an optional Aspen runtime adapter outside all Kiln core dependency graphs.
- Map `EventStorePort` operations to one Aspen durable atomicity domain per Kiln stream partition.
- Route approved process, transport, content, scheduling, and evidence effects through exact Aspen fabric-port bindings.
- Add a bounded native callback executable that consumes canonical input and emits canonical Kiln state and effect plans.
- Add a transport client for Radicle and operator trigger intake into the hosted Kiln service.
- Add typed Nickel profiles for cohorts, service identity, ports, bounds, authority, recovery, and non-claims.
- Add a visible composition root that selects Aspen instead of the local runtime profile.
- Preserve Lattice as the workflow provider, Mantle as the build provider, and Cairn as the gate provider.

## Dependencies

This change requires accepted Aspen changes that publish:

- a bounded execution fabric port backed by the reviewed `bounded-exec` component; and
- a production native-process system-extension host with canonical callback transport.

The integration must consume published immutable revisions. It must not depend on sibling worktree paths in product behavior.

## Impact

- **Core**: no Aspen dependency and no change to canonical CI meaning.
- **Contracts**: new Kiln-owned Aspen extension and client protocol.
- **Adapters**: new runtime, durable-state, content, transport, and effect mappings.
- **Shells**: new callback executable and Aspen-selecting service composition.
- **Configuration**: replacement of the profile-only `aspen-runtime-v1` declaration with an executable typed profile.
- **Testing**: positive and negative mapping, callback, durability, restart, transport, effect, and end-to-end conformance tests.

## Non-goals

- Move CI state meaning, fairness, retry policy, release policy, or provider selection into Aspen.
- Replace Lattice, Mantle, Cairn, Radicle, or Valence with Aspen-owned semantics.
- Import Aspen runtime types into `kiln-core`, `kiln-provider-api`, or `kiln-codec`.
- Grant process, filesystem, network, credential, or provider authority from artifact possession.
- Claim sandboxing, exact-once delivery, provider correctness, or production readiness from one passing fixture.
