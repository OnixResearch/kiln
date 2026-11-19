# Policy authoring contract

Basalt authors policy in Nickel (`contracts/policy.ncl`) and exports reviewed
JSON for the Rust consumer. `basalt::policy_projection` keeps the exported
projection meaning-compatible with the Rust DTO.

## Layered validation

Nickel owns reviewed source shape. Rust owns untrusted-artifact admission. The
two layers are complementary, not competing:

- **Nickel authoring contract** (`contracts/policy.ncl` and the shared prelude
  `contracts/policy-prelude.ncl`) rejects invalid reviewed intent before
  projection. It checks the exact schema, supported backend ids, normalized
  ids, non-empty unique allowlists, map-key/id alignment, and lexical
  resource/ability values.
- **Rust projection core** (`basalt::policy_projection`) rejects every external
  projection authoritatively, whether or not it came through Nickel.

## The shared prelude

`contracts/policy-prelude.ncl` provides reusable contracts and predicates:

- exact schema literal
- normalized ids
- supported backend ids (`nickel`, `steel`)
- non-empty unique allowlists
- map-key/id alignment
- lexical resource-prefix and ability values
- deterministic issue classes

Nickel validates resource and ability values only to the bounded lexical
preconditions shared with the Rust parser. It does not implement UCAN wildcard
containment, attenuation, caveat, proof, replay, token, or authorization
semantics.

## Projection parity

`basalt::policy_projection::validate_projection` is a pure deterministic core.
It validates a loaded `Policy` against the Rust consumer's supported contract:

- exact schema
- at least one contract
- contract and backend map keys equal their embedded ids
- unique identities
- supported backend vocabulary
- non-empty unique resource-prefix and ability allowlists
- bounded lexical resource and ability values

The core performs no I/O and no Nickel evaluation. A thin shell owns file reads
and Nickel invocation. Runtime enforcement never evaluates Nickel.

## Fixtures

`contracts/policy-authoring/fixtures/` covers both entry paths:

- `positive/valid-minimal.ncl` — a valid source policy exports.
- `negative/*.ncl` — key-id mismatch, empty allowlist, unknown backend,
  duplicate entry, and malformed ability are rejected by Nickel.
- `external-json/*.json` — the same faults, supplied as external JSON, are
  rejected by `parse_policy_json` and `validate_projection`.

The integration test `tests/policy_projection.rs` replays the external JSON
cases and proves the generated artifact stays compatible.

## Rust API

```rust
use basalt::{parse_policy_json, policy_projection::validate_projection};

let policy = parse_policy_json(basalt::DEFAULT_POLICY_JSON)?;
let verdict = validate_projection(&policy);
assert!(verdict.is_accepted());
```

## Non-claims

Nickel contract and projection parity conformance does not authorize a request,
validate a UCAN token, establish backend behavior, or replace Rust receipt
authority. It proves only that the reviewed policy shape and projection match
the supported Rust consumer.
