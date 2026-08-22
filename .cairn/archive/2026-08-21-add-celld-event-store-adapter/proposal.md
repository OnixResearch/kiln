# Add a celld event-store adapter

## Why

Kiln defines an `EventStorePort` and an in-memory reference journal. It has no distributed event-store adapter for a replaceable service shell.

`denoland/celld` runs Workers and Durable Objects on an operator-owned fleet. Each object has one SQLite database and one fenced writer.

A purpose-built Worker can keep one Kiln journal partition in one Durable Object. It can atomically store events and linked outbox records.

This route is narrower than a general celld integration. celld does not supply CI workflow execution, build execution, global fairness, or release authority.

## What Changes

- Add an optional `kiln-adapter-celld` implementation of the application-owned `EventStorePort`.
- Add a versioned Worker contract for journal, outbox, lease, replay, and reconciliation operations.
- Place one admitted journal partition in one named Durable Object.
- Store accepted events and linked outbox effects in one Durable Object SQLite transaction.
- Preserve expected revisions, canonical bytes, Kiln identities, celld identities, and provider observations separately.
- Require an exact celld release, Worker deployment, storage cohort, output-gate policy, and storage-probe result.
- Use typed Nickel for adapter, transport, partition, limit, authentication, migration, and non-claim policy.
- Treat transport loss after possible commit as unknown until an idempotency lookup resolves it.
- Add positive, negative, interruption, takeover, migration, authentication, bound, and drift fixtures.

## Research Baseline

The initial compatibility baseline is `denoland/celld` release `v0.2.1` at revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da`.

The release uses Apache-2.0. Its Linux release assets publish SHA-256 digests and GitHub build attestations.

celld documents Durable Object SQLite transactions, one-writer fencing, and durable acknowledgements when `CELLD_OUTPUT_GATE=1`.

celld also documents important limits. Workflows are planned, the internal operator API is alpha, and the fleet does not authenticate public applications.

## Dependencies

Implementation starts after `align-provider-port-ownership` establishes application-owned ports and distinct infrastructure errors.

The accepted `durable-operations` specification supplies journal, outbox, replay, lease, recovery, and non-claim meaning.

The local SQLite adapter remains independent. Both adapters can reuse provider-neutral storage decisions without sharing provider code.

## Non-Goals

- Using celld as a workflow, build, placement, or tenant-fairness provider.
- Calling the unauthenticated internal operator API from normal `EventStorePort` operations.
- Writing celld fleet-bucket objects or private deployment records directly.
- Importing celld, V8, Wrangler, object-store, or Durable Object types into Kiln core crates.
- Providing atomic transactions across two Durable Objects.
- Treating a celld deployment, response, or storage receipt as CI success or release evidence.
- Supporting hostile multi-tenant co-location on the initial celld alpha cohort.

## Impact

- **Affected specs:** new `celld-event-store-adapter` capability and the accepted `durable-operations` contract.
- **Application contract:** the application-owned `EventStorePort` remains provider-neutral.
- **Adapters:** add a bounded Rust client and a purpose-built Worker artifact.
- **Configuration:** add typed celld cohort, transport, partition, authentication, limit, and migration policy.
- **Testing:** add pure model tests, Worker tests, client tests, and exact-cohort fleet fixtures.
