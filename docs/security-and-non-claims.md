# Security and non-claims

Kiln validates bounded facts. It does not create ambient trust or external authority.

## Trigger intake

A trigger shell validates the protocol signature before it creates a command. The core receives only bounded verification facts.

Duplicate policy uses native delivery identity and retained journal history. Duplicate suppression does not prove exactly-once delivery.

## Provider effects

Capability admission occurs before dispatch. The selected provider cannot change without a new admitted command.

A transport failure after acceptance produces `Unknown`. Reconciliation or operator policy must resolve this state.

## Remote execution

Remote protocol digests stay separate from Kiln BLAKE3 identities. Cache hits remain provider observations.

Remote success requires every declared output. It does not prove hermeticity, sandboxing, cache integrity, or product correctness.

## COB effects

A COB mutation plan binds exact current heads. The shell must reload the object before signing or writing.

Concurrent heads remain visible. Signatures do not prove actor trust or action authority.

## Logs and diagnostics

Log bytes have stream, segment, total, read, and retention bounds. Gaps and truncation remain explicit.

Diagnostics use safe fields and redaction markers. Kiln cannot prove that external output contains no secret before capture.

## Interchange

SLSA, in-toto, CDEvents, CloudEvents, and OpenTelemetry remain projections. Schema validity does not transfer builder, signer, lifecycle, or release authority.

## Repository hosting

Radicle is planned governance, not a core dependency. Local validation performs no Radicle or mirror mutation.

Matching acquisition routes do not prove future availability, delegate honesty, source correctness, or CI correctness.
