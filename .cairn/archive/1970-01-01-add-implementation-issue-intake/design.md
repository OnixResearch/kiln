## Decisions

Authentication reuses the adapter's existing constant-time HMAC verification and
header validators; issue intake adds no parallel crypto path. Payload decoding
happens only after authentication, and selected-field deserialization rejects
duplicate fields.

The work sequence is the issue `updated_at` timestamp parsed as strict
`YYYY-MM-DDTHH:MM:SSZ`. Kiln does not accept local offsets or loose date forms;
malformed timestamps fail admission.

The source version is BLAKE3 over the exact delivery bytes. That makes duplicate
deliveries idempotent and edits distinct, and the work ledger orders them by the
monotonic sequence rather than the digest.

Closing returns a typed closed observation instead of a work record. Reopening
admits a new version. Neither proves implementation completion; cancellation and
supersession semantics remain with the future claim consumer.

## Non-claims

This adapter authenticates and normalizes delivery. It does not poll GitHub,
persist a work ledger, claim work, execute a worker, verify a candidate, publish
a pull request, or modify issue state. Issue text cannot add acceptance
obligations or permissions.
