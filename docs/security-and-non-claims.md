# Security and non-claims

Kiln validates bounded facts. It does not create ambient trust or external authority.

## Trigger intake

A trigger shell validates the protocol signature before it creates a command. The core receives only bounded verification facts.

Duplicate policy uses native delivery identity and retained journal history. Duplicate suppression does not prove exactly-once delivery.

## Provider effects

Capability admission occurs before dispatch. The selected provider cannot change without a new admitted command.

A transport failure after acceptance produces `Unknown`. Reconciliation or operator policy must resolve this state.

## Authentication hardening

The deployed Celld Worker, not its in-memory Rust model, owns the five-minute
timestamp window and durable mutation nonce rejection. It verifies HMACs through
WebCrypto; the Rust protocol uses the HMAC library's verification primitive.
An exact-operation retry needs a fresh nonce. Signed reads remain reusable only
within the timestamp window; they are not a consume-once protocol. See
[the Celld transport boundary](celld-event-store.md#transport-and-authentication).

Public operator keys, signed request fields, and public BLAKE3 identities do not
require secret-dependent constant-time equality. The local all-zero seed
admission check is not a remote authentication oracle. These comparisons do
not justify replacing ordinary equality with handwritten cryptographic code.

The standalone Aspen host requires an owned private final socket directory and
uses an owner-only socket. Clients must run as the same user; stable ancestor
directories remain operator-owned provisioning. See
[host startup requirements](../runtime/kiln-aspen-host/README.md#command).

Registry advisory results describe the resolved root lock, not application
exploitability, independently pinned runtime dependencies, or a newly qualified
deployment. Compatible updates and residual cohort constraints are recorded in
[reference pins](reference-pins.md#registry-execution-transport-cohort).


## Remote execution

Remote protocol digests stay separate from Kiln BLAKE3 identities. Cache hits remain provider observations.

Remote success requires every declared output. It does not prove hermeticity, sandboxing, cache integrity, or product correctness.

## COB effects

A COB mutation plan binds exact current heads. The shell must reload the object before signing or writing.

Concurrent heads remain visible. Signatures do not prove actor trust or action authority.

## Logs and diagnostics

Log bytes have stream, segment, total, read, and retention bounds. Gaps and truncation remain explicit.

Diagnostics use safe fields and redaction markers. Kiln cannot prove that external output contains no secret before capture.

## Semantic merge checks

A semantic merge result applies only to the checked property for one exact role-tagged quartet (base, variants, candidate) and its assumptions, dependencies, model, checker, and policy. It does not prove source trust or grant merge, publish, status, or release authority. Unknown and other non-verified outcomes are neither success nor a demonstrated conflict. See `docs/semantic-merge-orchestration.md`.

## Interchange

SLSA, in-toto, CDEvents, CloudEvents, and OpenTelemetry remain projections. Schema validity does not transfer builder, signer, lifecycle, or release authority.

## Repository hosting

Radicle is planned governance, not a core dependency. Local validation performs no Radicle or mirror mutation.

Matching acquisition routes do not prove future availability, delegate honesty, source correctness, or CI correctness.
