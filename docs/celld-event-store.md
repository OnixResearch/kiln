# celld event-store adapter

Kiln can persist an event journal to a versioned Kiln Worker deployed on a
`denoland/celld` fleet. Each admitted journal partition maps to one named
Durable Object. The Worker stores an accepted event batch and every linked
outbox record in one SQLite transaction.

Kiln remains the source of journal, outbox, lease, replay, fairness, and effect
meaning. celld supplies durable object persistence only.

## Pinned cohort

| Component | Pin |
|---|---|
| `celld` release | `v0.2.1` |
| `celld` revision | `ae8fac053d79f971bfcb996054bb43eb2f9b05da` |
| License | Apache-2.0 |
| Worker protocol | `kiln-celld-event-store.v1` |
| Store schema | `1` |
| Auth | `hmac-sha256` |

The release publishes Linux assets with SHA-256 digests and GitHub build
attestations. The fleet may gate writes with `CELLD_OUTPUT_GATE=1`.

## Authority boundary

The adapter implements the application-owned `EventStorePort`. It does not use
the celld internal operator API (`/state`, `/do/`, `/evict/`, `/shutdown`) for
normal event-store operations. It does not write fleet-bucket objects or claim
hostile multi-tenant isolation, global serializability, or exactly-once
external effects.

## Partitioning

Each stream maps deterministically to one named Durable Object through a
BLAKE3 partition window on the explicit versioned projection. One mutation
never spans two objects. celld placement and activation order do not become
Kiln fairness or dispatch authority.

## Atomic append

A Worker append admits the batch against the stored stream revision, event
identities, and named bounds. On admission it commits the events, the new
revision, the linked outbox records, and the idempotency result in one SQLite
transaction. An interrupted transaction exposes no partial state.

## Transport and authentication

Requests use one admitted HTTPS origin and path. Redirects and request-supplied
destinations stay disabled. HMAC-SHA256 binds the path, version, key
reference, timestamp, nonce, operation identity, payload identity, and body.
An ambiguous outcome stays `Unknown` until the exact operation is reconciled
before redispatch.

## Replay, leases, and migrations

Replay checks framing, continuity, canonical identities, and versions before
state evolution. Leases keep outbox owner, attempt, and completion facts across
eviction and restart. Schema migrations use versioned pure plans and one-object
transactions; unknown, partial, and cross-object migrations fail closed.

## Fixtures

The maintained fixtures cover profile admission, partition mapping, atomic
append, idempotent repeats, payload conflicts, authentication, timeout,
disconnect, malformed responses, redirects, TLS failure, key mismatch, stale
revisions, duplicate identities, damaged history, and migration edges.

## Non-claims

- A celld acknowledgement does not prove external effect execution.
- One-object transactions do not provide global serializability.
- Alpha-cohort admission does not prove hostile tenant isolation.
- Worker responses do not prove CI success or provider correctness.
- Storage-probe results do not prove object-store durability.
- Replay through production `evolve` emits no provider effect.
