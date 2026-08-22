# Publish the Radicle source acquisition contract

## Why

Kiln cannot enable Radicle source acquisition without an immutable contract for signed-reference observation and bounded snapshot evidence.

The selected Radicle cohort verifies `refs/rad/sigrefs`, repository identity roots, and parent-linked signed-reference history. Kiln needs a runtime-independent contract that preserves these facts without importing Radicle, Git, storage, or network code into its core.

## What Changes

- Add a standalone `no_std + alloc` Radicle source contract crate.
- Define versioned request, signed-reference observation, snapshot evidence, and failure models.
- Require one exact RID, signer namespace, branch reference, expected signed-refs commit, and expected revision.
- Require the Radicle `parent` signed-reference feature level.
- Bind canonical snapshot evidence with BLAKE3 while preserving native Git identities.
- Add typed Nickel contracts and positive and negative producer fixtures.
- Document external cohort sources, claim limits, and consumer obligations.

## Impact

- **New component**: `kiln-radicle-source-contract`.
- **Runtime boundary**: no Git, Radicle runtime, process, network, filesystem, clock, or credential dependencies.
- **Consumers**: Radicle shells can pin one reviewed contract revision before enabling source acquisition.
- **Claims**: contract admission proves structural consistency only. It does not prove source trust, freshness, completeness, or remote availability.
