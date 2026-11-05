## Context

Kiln's Nix build request carries only a `.drv` path and required output names
(`kiln-adapter-nix/src/wire.rs`); the composition hashes the file but never
parses it. Product observations carry only Kiln identities. Mantle already
vendors Snix below its AGPL boundary; Kiln must consume equivalent facts without
linking GPL code.

## Decisions

### Decision: Apache-2.0 facts crate

Use `nix-derivation` 0.6.x, `nix-archive` 0.6.x and `nix-narinfo` 0.4.x behind a
new `kiln-nix-store` pure crate. It performs no ambient I/O. It owns bounded
derivation admission, NAR hashing/reference scanning, StorePath/CA normalization,
and narinfo rendering. Adapters supply exact bytes and paths.

### Decision: structural derivation admission, explicit limits

Parse the `.drv` with `nix-derivation`, validate structure, require the profile
platform to equal `system`, require every requested output name to exist, and
compare any declared output path with the task/store binding. Input-addressed
output paths that require input-modulo hashes are admitted as task-bound
expectations, not recomputed. The admission records the exact derivation bytes
identity and the input source/derivation sets; it does not claim Nix-equivalent
path calculation where input hashes are unavailable.

### Decision: additive observations

Add optional `constraints` to `BuildEffectPlan`, optional `store` to
`BuildProductObservation`, and optional `inputs`/`input_closure_identity` to
`BuildEvaluationObservation`. Absent fields serialize and replay unchanged.
Store facts use native SHA-256/NAR spellings; Kiln BLAKE3 identities stay
separate.

### Decision: honest reference completeness

Reference scanning uses 32-byte Nix-base32 hash parts for every candidate the
adapter can prove: input sources, known input derivation paths, and every output
path. The observation records matches and a `complete` flag. It is false when
the candidate set omits unresolved input-derivation outputs. The scanner is
not claimed byte-identical to C++ Nix beyond the supplied vectors.

### Decision: cache projection is static and read-only

`kiln-nix-store cache` writes `nix-cache-info`, `.narinfo` records and NAR bytes
from an explicit observation/output-path document. It does not serve HTTP,
sign with operator keys, substitute, or mutate a store. Existing NARs may be
copied; otherwise the command encodes the exact path under bounded limits.

## Risks

- NAR encoding of a mutable output tree is a snapshot observation; concurrent
  mutation is detected where the encoder reports changed sizes but is not an
  atomic snapshot.
- Input-addressed output paths are not recomputed without input hashes.
- Reference candidates are partial when unresolved inputs exist; the explicit
  completeness flag prevents overclaiming.
- GPL Snix crates remain out of Kiln; Mantle retains its vendored data layer.
