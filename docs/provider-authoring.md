# Provider authoring

A provider implements only the narrow ports that it supports. It does not implement one universal resource interface.

## Admission

Each profile names these facts:

- contract version;
- provider profile identity;
- role and identity namespace;
- supported operations and outcomes;
- request, result, and in-flight bounds.

Kiln rejects an unavailable selected provider. It does not select another provider without a new command.

## Effect linkage

Each dispatch binds the Kiln effect, attempt, profile, operation, and idempotency key. Preserve every available native reference.

Return `Unknown` when transport failure leaves provider execution uncertain. Do not return success or safe retry without provider evidence.

## Source providers

Keep discovery, resolution, acquisition, and mutation separate. Bind evaluations to exact immutable revisions.

A Jujutsu profile must preserve operation ID, change ID, commit ID, bookmark state, and backend kind. Reject ambiguous or divergent resolution.

## COB providers

Select one exact profile before semantic mapping. Unknown types can use only an explicit bounded opaque-read policy.

Bind each mutation plan to current heads. Reload the object before signing or writing.

Keep every concurrent head visible. Do not use last-writer-wins behavior.

## Conformance

Add positive and negative tests together. Test unsupported capabilities, stale links, duplicate callbacks, timeouts, cancellation, and unknown outcomes.

Run:

```console
cargo test -p kiln-conformance
```
