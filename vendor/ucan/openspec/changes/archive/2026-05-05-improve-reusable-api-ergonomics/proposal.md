## Why

The current public API is usable but still example-oriented. Reusable crates need frictionless parsing, serialization, builders, time helpers, and integration examples. Without these, downstream crates will wrap `ucan` with local newtypes and helper layers, creating divergent idioms.

This is the fifth-priority follow-up for the currently available token issue/verify surface. Ergonomics for authorization, resolver/proof-store, and invocation APIs are covered by those feature-specific changes when those APIs land.

## What Changes

- Add `FromStr`, `Display`, and serde support for public wrapper types where safe.
- Add builders for issue and verify inputs to avoid positional mistakes.
- Add std-shell time helpers based on `SystemTime`/`Duration` while keeping core logic deterministic.
- Add real integration examples for HTTP bearer parsing/verification, structured failure handling, and reusable-library onboarding.

## Non-Goals

- Add a full CLI product in this change.
- Add network proof fetching or remote DID resolution.
- Change token semantics merely for ergonomic reasons.

## Capabilities

### New Capabilities
- `library.ergonomics.wrapper-traits`: Public semantic wrapper types can cross string/JSON boundaries without ad-hoc downstream code.
- `library.ergonomics.builders`: Public workflows can be constructed without long positional argument lists.
- `library.ergonomics.time-helpers`: Consumers can build verification times and token bounds from std time types without hidden clocks in verification.
- `library.ergonomics.integration-examples`: Consumers can copy realistic patterns for HTTP bearer verification and structured failure handling.

### Modified Capabilities
- `library.ergonomics.integration-examples`: Onboarding grows from smoke examples to reusable application examples for the current issue/verify surface.

## Impact

- **Files**: `src/**`, examples, README, `docs/public-api.md`, tests.
- **APIs**: Adds trait impls, builders, time helpers, and examples; may deprecate positional constructors only after builders exist.
- **Dependencies**: May require enabling serde impls on public types; no core dependency creep.
- **Testing**: Needs positive and negative parse/serde/builder tests and compile/run checks for integration examples.
