# UCAN/Basalt authority core

`basalt::authority_core` is a pure deterministic admission boundary over already-loaded Basalt policy values and caller-supplied references to already-verified UCAN facts. It admits only one exact contract/resource/ability/request binding.

## Ownership boundary

The caller-owned shell remains responsible for:

- compact-token parsing and structural checks;
- signature verification and DID/key resolution;
- proof loading, proof-chain validation, attenuation, and verification time;
- revocation freshness and backend availability;
- caveat policy evaluation;
- replay-store admission and durability;
- loading Nickel-exported policy JSON and establishing its artifact reference.

The authority core performs no filesystem, network, environment, clock, storage, process, or Nickel evaluation. A `blake3:...` reference is a typed identity field, not proof that the referenced verification happened or remains fresh.

## Pure input and decision

`AuthorityInput` contains:

- an exact contract ID, resource, ability, and request reference;
- verified direct or delegated grant references;
- UCAN verification receipt references binding request and grant references;
- exactly one shell-trusted Basalt policy reference;
- caller-supplied caveat and replay dispositions;
- exactly one `exact-request` authority claim;
- the complete non-claim boundary.

`evaluate_authority` compares the input policy reference to the separate trusted reference supplied with the loaded `Policy`. It rejects malformed or mismatched references, overclaims, denied caveat/replay dispositions, missing or mismatched grants, missing/stale/rejected receipts, and receipt request/grant mismatches before calling Basalt's existing `enforce` function with the selected exact grant.

An allowed `AuthorityDecision` carries the exact request binding and the supported claim `exact-request-only`. A denied decision carries no binding and one stable first-failure diagnostic.

## Existing request adapter

`adapt_enforcement_request` preserves the existing public `EnforcementRequest` contract, resource, ability, and capability fields. Existing capabilities become verified grant facts only when the caller supplies exactly one `VerifiedGrantBinding` per capability. A count mismatch fails before an `AuthorityInput` is produced.

The adapter does not upgrade an unverified `CapabilityGrant` into authority by itself. Raw token material is not an authority-input field and is rejected by the DTO's unknown-field boundary.

## Fixture matrix

Nickel-authored fixtures live under `contracts/authority-core/fixtures/`.

Positive cases:

- exact direct grant and current matching verification receipt;
- exact delegated grant and current matching verification receipt.

Fail-closed cases:

- missing grant;
- wrong resource;
- wrong ability;
- stale verification reference;
- caveat denial;
- replay denial;
- missing policy reference;
- malformed request reference;
- token-possession authority overclaim;
- missing verification receipt;
- verification/request reference mismatch.

Contract-negative cases reject weakened non-claims, unsupported dispositions, and incompatible expected results.

## Explicit non-claims

Passing authority-core validation does not grant or prove:

- transport authority;
- storage authority;
- persistence authority;
- replay-store truth;
- key-resolution trust;
- downstream business authority;
- release eligibility;
- whole-system safety.

It also does not establish UCAN cryptographic correctness, receipt provenance, revocation freshness, or policy completeness. Those remain external inputs and caller-owned responsibilities.

## Validation

From the Basalt repository root:

```bash
nix build path:$PWD#checks.$(nix eval --raw --impure --expr builtins.currentSystem).authority-core-fixtures --no-link -L --option secret-key-files ''
nix develop -c env CARGO_TARGET_DIR=/tmp/basalt-authority-target RUSTC_WRAPPER= CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked --test authority_core --no-default-features
```

The direct release verifier and `nix flake check path:$PWD -L` include fixture freshness, runtime replay, package-content, Clippy, and documentation rails.
