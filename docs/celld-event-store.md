# celld event-store adapter

Kiln owns a versioned Worker entry at `src/index.mjs` for the SQLite Durable
Object API in its pinned `denoland/celld` cohort. The local SQLite tests execute
the entry in Node; `tests/celld-live-cohort.sh` additionally builds the
hash-pinned v0.2.1 archive, deploys that Worker to a loopback-only pinned Celld
node with SeaweedFS S3, output gating and the conditional-write startup probe
enabled, and exercises the real Rust `EventStorePort` through HTTPS with an
explicit, temporary CA. Those are local observations, not a fleet release gate.

Kiln owns journal, outbox, lease, replay, fairness, and effect meaning. The
Worker is a persistence boundary, not dispatch or operator authority.

## Pinned cohort

| Component | Pin |
|---|---|
| `celld` release | `v0.2.1` |
| `celld` revision | `ae8fac053d79f971bfcb996054bb43eb2f9b05da` |
| License | Apache-2.0 |
| Worker protocol | `kiln-celld-event-store.v1` |
| Store schema | `1` |
| Auth | `hmac-sha256` |

The `celld-cohort` Nix output pins the source archive. The local cohort runner
builds it with its upstream locked Cargo dependencies and the flake-locked
toolchain, checks `celld --version`, starts Celld with
`CELLD_OUTPUT_GATE=1` and `CELLD_STORAGE_PROBE=1`, and only sends an append
once the Worker can respond over verified loopback TLS. A failed storage
conditional-write probe prevents Celld startup; the local SeaweedFS backend
passed that probe. The same result is not proof of arbitrary object-store
durability or a distributed fleet takeover.

At each local startup the trusted operator shell observes domain-separated
BLAKE3 content identities for the actual binary
(`kiln.celld.artifact.v1`), bundled Worker
(`kiln.celld.worker-bundle.v1`), and ingress CA PEM
(`kiln.celld.ingress-ca.v1`), plus the platform, deployed version, S3 bucket
and endpoint, HTTPS origin, pinned release and source revision, Worker
protocol/schema declarations, and the Celld process gate and startup probe.
The upstream source archive retains its Nix-required SHA-256 SRI; HMAC-SHA256
remains the Worker authentication protocol.
`CelldProfile` compares these fresh (at most five-minute)
observations against the selected instance before every request; missing,
stale, or contradictory observations reject before dispatch.

`HttpCelldRuntime` hashes the actual supplied TLS root PEM once with the same
`kiln.celld.ingress-ca.v1` BLAKE3 domain, and rejects each request before
network dispatch unless its installed PEM matches both selected and observed
ingress digests.

The real Worker also returns its deployed version, revision, protocol, and schema on successful
TLS-authenticated responses, and Rust rejects a differing response as Unknown.
The runner checks that independent response boundary, too. These are scoped
operator measurements and Worker declarations, not cryptographically signed
Celld node attestation; they cannot admit a separate unobserved remote fleet.

## Authority boundary

The adapter implements the application-owned `EventStorePort`. It does not use
the celld internal operator API (`/state`, `/do/`, `/evict/`, `/shutdown`) for
normal event-store operations. It does not write fleet-bucket objects or claim
hostile multi-tenant isolation, global serializability, or exactly-once
external effects.

## Partitioning

The Rust projection maps a stream deterministically to a named Durable
Object. The Worker routes a signed single-stream batch to its named object;
the live runner verifies one local partition but does not prove ownership
takeover or cross-node durability.

## Atomic append

The Worker enforces bounded batches, contiguous revisions and a stored
protocol/schema version. Its one-object SQLite transaction writes events,
outbox records, revision, nonce, and operation outcome. A local injected
SQLite abort after event writes rolls back all of them; file close/reopen
retains an accepted batch. The local pinned Celld runner checks that a
gated, acknowledged append returns its exact event and outbox over an
authenticated Rust read, then reconciles an injected lost response.

## Transport and authentication

The Worker checks HTTPS, its declared append/read/reconcile/lease/migrate
paths, HMAC-SHA256 with length-prefixed fields, timestamp, nonce, and request
bounds before resolving the object; private Celld control routes are rejected.
Missing or disabled `KILN_OUTPUT_GATE` and `KILN_STORAGE_PROBE` Worker
declarations, or a missing deployed-version declaration, reject a signed
request before object resolution. The operator must supply these Worker
declarations from the actual Celld process settings and completed deployment;
a static Wrangler manifest is not evidence of node configuration.
`HttpCelldRuntime` is optional and owns an explicit TLS 1.3 trust root,
zeroizing HMAC key, one admitted HTTPS origin, disabled ambient proxies and
redirects, a deadline, and named request/response bounds. The deployment
template uses `origin = "https"` as the Worker-side scheme check; the Rust
profile requires the complete trusted origin at composition. On lost append
response, the adapter reports `Unknown`; a subsequent call performs a signed
exact-operation lookup before any redispatch. Caddy's test-only root is passed
to the client without adding it to host trust.

The sibling stack-owned `bounded-http` API would supply stronger general
outbound mechanics and accepts caller-supplied DER roots, but the present
workspace declares Rust `1.96.0` and `publish = false`, whereas Kiln declares
Rust `1.88`; its only local remote is `rad://` and no published immutable Cargo
source is available here. A sibling-path dependency is not durable. The
Celld-owned shell is therefore isolated instead of changing Kiln's supported
toolchain or copying the sibling product-neutral implementation.

## Replay, leases, and migrations

Local Worker reads inspect bounded ordered SQLite rows; the Rust
`EventStorePort::load` sends a signed read and checks sequence, stream, and
event BLAKE3 identities before the returned history can be evolved by
`replay_checked` through production core. Missing or malformed responses
fail closed.

The signed `/v1/lease` route atomically compares the exact persisted
`StoredLease` before changing owner, deadline, attempt, idempotency, completion,
or reconciliation evidence. Pure Rust plans decide both initial claims and
terminal reconciled completion; the Worker does not contact providers.
`lease_recovery_action` requires reconciliation for expired or ambiguous
outcomes. The local cohort exercises an active lease after Celld node restart,
then persists non-null terminal evidence across another restart. This is
one-node persistence, not a claim of distributed ownership takeover.

The signed `/v1/migrate` route distinguishes an absent stream from separately
initialized prior empty schema `0`. The sole admitted migration is empty
`0 -> 1`, with exact stored BLAKE3 schema identities and a provenance row
written in the same SQLite transaction; unknown edges, wrong inputs, ledger
faults and partial histories fail closed. A prior schema-1 journal without
the identity ledger is not silently reinterpreted. The local cohort checks
the real migration and refuses an incompatible schema declaration before
a mutation, then resumes after the compatible process returns. General
non-empty schema migrations and multi-node mixed-fleet routing are not claimed.

## Fixtures

The executable local Worker fixture is `node --test
tests/celld-worker.test.mjs` (Node with `node:sqlite` and `b3sum`). It verifies
signed append/repeat, event/outbox/schema/operation persistence, injected SQL
rollback, signature/nonce/private-route rejection, request bounds, identity
damage, conditional lease conflict and evidence retention, and valid/invalid
schema migration including injected ledger failure. Rust
`nix develop --offline --command cargo test -p kiln-adapter-celld --lib`
verifies pure admission, replay, profile, partition, lease, migration and
fail-closed client outcomes. The in-memory Rust Worker fixture is only a
model.

The executable dependency gate is `nix build --offline
path:$PWD#checks.x86_64-linux.celld-boundaries --no-link`. Inside its isolated
Nix build sandbox it runs actual `kiln-core`, `kiln-provider-api`, and
`kiln-standalone` unit tests and checks their resolved normal Cargo dependency
trees without Celld, Worker/V8, esbuild or object-store credentials. A separate
disposable workspace compiles a real forbidden implementation-type import into
core; the same dependency gate rejects it and reports the offending Cargo
edge. This does not mutate the source workspace.

The complete loopback integration command is `./tests/celld-live-cohort.sh`.
It verifies the archive digest, builds pinned Celld v0.2.1, measures its
binary, bundled Worker, deployment version, storage cohort, CA and startup
flags, and exercises ignored `tests/live_worker.rs` and
`tests/live_control.rs` through strict Rust HTTPS. A contradictory served
Worker deployment version fails response admission.

The live Rust path rejects both agreeing-but-forged operator ingress digests
and a byte-different valid TLS root PEM before signed Worker dispatch.

Positive phases enable the local output gate and conditional-write probe; phased restarts verify
SQLite lease/schema facts and terminal evidence, while incompatible schema
and separately disabled-gate observations reject canonical append before
Worker dispatch. It uses temporary storage, scoped CA, secret, separate
loopback public/internal listeners, and a cleanup trap; neither a sibling
checkout nor the host's unrelated celld v0.3.0 binary is used.

## Non-claims

- A celld acknowledgement does not prove external effect execution.
- One-object transactions do not provide global serializability.
- Alpha-cohort admission does not prove hostile tenant isolation.
- Worker responses do not prove CI success or provider correctness.
- Storage-probe results do not prove object-store durability.
- Replay through production `evolve` emits no provider effect.
