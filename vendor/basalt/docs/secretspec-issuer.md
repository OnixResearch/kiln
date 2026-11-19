# SecretSpec-Backed UCAN Issuer Key Boundary

Basalt resolves the UCAN issuer signing key through the pinned SecretSpec Rust
SDK. This document records the reviewed dependency identity, the metadata
declaration, the bounded imperative-shell boundary, and the claim limits of
this surface.

## Reviewed dependency pin

Basalt pins SecretSpec `v0.17.0`.

The `secretspec` dependency resolves to the exact crates.io release whose
source corresponds to the reviewed upstream tag `v0.17.0`:

- tag commit: `a8794e46ec9664a0e1a3869cc3105d0853937e48`
- license: Apache-2.0

`Cargo.toml` pins `secretspec = { version = "=0.17.0", ... }` and `Cargo.lock`
records the resolved release. The release verifier and the immutable identity
module `src/secretspec_issuer_identity.rs` bind this revision. Builds resolve
the durable, reviewed boundary rather than a moving head.

SecretSpec is Apache-2.0. Basalt remains `AGPL-3.0-or-later`. The pin does not
host secrets in the repository.

## Metadata declaration

A metadata-only declaration names the UCAN issuer signing key under the
reviewed `basalt-oidc-exchange` scope and an explicit profile. The declaration
carries no key value.

Positive and negative fixtures live in `contracts/secretspec-issuer/fixtures/`.
The positive fixtures cover a clean key load and a rotation overlap window.
The negative fixtures cover missing required keys, missing declared keys, the
wrong scope, and provider failure. The unit and integration suites exercise the
bounded resolution and rotation paths through the pure core.

## Bounded imperative shell

Key resolution runs only in the repository/service shell. The shell reads the
declared metadata and asks the pinned `secretspec::resolve_json` boundary to
load the value from the selected provider.

The SDK call runs inside an owned worker with an explicit timeout and
output-size bound. `src/secretspec_issuer/shell.rs` owns provider access,
timeout enforcement, chunking to the output bound, and failure plumbing. The
pure core (`classify_resolution`, `validate_declaration`, and
`validate_rotation_window`) never reads clocks, environment, files, network
state, or secret keys.

Write operations are not enabled. Plaintext value caching is disabled. The
request requests only the value bound to the declared scope and key identifier.

## Rotation

Rotation selects a new explicit key identifier and overlaps the public
verification-key window without exposing private signing material.
`RotationWindow` carries only public verification digests and a signing key
identifier; `validate_rotation_window` bounds the overlap count and rejects
private material.

## Claim boundary

This change proves only that a reviewed SecretSpec boundary can resolve a
declared issuer key. It does not verify OIDC tokens, issue UCAN capabilities,
or prove that any provider is a correct secret authority. Missing, wrong-scope,
timeout, oversized-output, and provider-failure outcomes disable issuance and
never fall back to a static or ambient key.
