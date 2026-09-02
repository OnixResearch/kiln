# Cross-consumer source inventory

This inventory fixes the extraction cohort. Nix SRI BLAKE3 identities bind exact reviewed file bytes; repository revisions bind surrounding source context. Exact pre-extraction statement bytes and identities are frozen in [`baseline-vectors.md`](baseline-vectors.md).

## Source cohort

| Consumer | Revision | Reviewed source | BLAKE3 SRI |
| --- | --- | --- | --- |
| Cairn | `cc4639be24475d80844b091cdec605eb2fef9ff0` | `crates/cairn-core/src/verified/authenticated_inputs.rs` | `blake3-q5+vjpOmWffVAADyi/iyYNfVIO7nztBPwwbkGczSD2A=` |
| Cairn | same | `crates/cairn-cli/src/authenticated_inputs.rs` | `blake3-r0JO4LKpiwBLoYp3gYh0bMJ+07d/uf/QzsucBkmGznw=` |
| Cairn | same | `docs/authenticated-evidence-readiness.md` | `blake3-jNd0bM1bUttMfZeQ3NZqxlC9yJnZ7iytjM0CFDFDayg=` |
| Mantle | `26c6880c08cd730c9e9420f92ea91a87212e492d` | `src/oci_registry.rs` | `blake3-9HMsWm7BZ3svVPfP74Ni+CAMbJBxnAcBcFjDizac+NU=` |
| Mantle | same | `src/oci_registry_shell.rs` | `blake3-korPoeTzq0BTeqOYLwQBr5zEDoLu55UVc5NjAoRo8sw=` |
| Mantle | same | `lib/oci_registry_trust.ncl` | `blake3-3rdy6fA6tKiVHcjwy6Lj3ISrEf9Z9nB6Ahor6A6pA2I=` |
| Mantle | same | `crates/crunch-action-result-core/src/lib.rs` | `blake3-zNKcOlw39YRLuDnAtEjqY8Oc/oujB9c2Lwh5DUco/8k=` |
| Mantle | same | `crates/crunch-attestation-core/src/canonical.rs` | `blake3-yh178L8XtUiUVLXoLOSaHrWrBVN4z7AmA316zuovdsw=` |
| Mantle | same | `crates/crunch-attestation-core/src/policy.rs` | `blake3-Mpv/LpuqQwVP4+dlc3qfrOeBGZEItxL9yw8YXb+/kps=` |
| Mantle | same | `docs/kernel-bundle-oci.md` | `blake3-9GA6cgYSKG8vzba8nJ3kFBSIEDix7vJZS2MI2EObSLw=` |
| Molten | `70590459a218dc8e66948ab6f305a7c54142b710` | `crates/molten-core/src/fabric_crypto_identity/model.rs` | `blake3-SqG+i9gxpsjpjjvDE5AypTd6k+/X/yHCDizjyCVFyrs=` |
| Molten | same | `crates/molten-core/src/fabric_crypto_identity/admission.rs` | `blake3-ym2vBdHorhaU1iw7lYCOtuVjaCX7Mpk1gmFqLjlcDiU=` |
| Molten | same | `crates/molten-core/src/fabric_crypto_identity/rotation.rs` | `blake3-ofI0/P8JacZM4JRntjwiYszCqx9eFiHTSrW8OiSUiVc=` |
| Molten | same | `docs/fabric-cryptographic-identity.md` | `blake3-U2HCmI8Io9QIuCc0LJFeR1qOzGVrFN/2nXJ7Df7hvPA=` |
| Valence | `8eccac35fcd8aae8ef64b3d1021e7f1a53656180` | `crates/valence-core/src/cob_evidence.rs` | `blake3-q1iBjsYzVvY12HB6R96HGbjWGgDPjKPr54+Wktr7bPE=` |
| Valence | same | `crates/valence-radicle/src/transport.rs` | `blake3-Xv3lgqtytAoZEukHdsZ4xCEmfuE/pVIv5dSq1H3RlFc=` |
| Valence | same | `docs/radicle-cob-evidence.md` | `blake3-OmQ8yJUcsyFjO2iXavddFlHxve5mH68Dci5LjtJLxtk=` |

## Workspace migration sources

| Source repository | Revision | Imported material | License in this workspace |
| --- | --- | --- | --- |
| `OnixResearch/artifact-auth` | `d42755d29bf12b5c14448e522aaa42cb9c47d310` | Repository history, authentication crates, fixtures, typed config, lifecycle, and release rails | `MIT OR Apache-2.0` |
| `OnixResearch/artifact-transfer` | `507b10823c4513064710a70fe565f9a1ee592985` | `artifact-transfer-core`, extraction design, adoption gate, and disposable pilot evidence | `MIT OR Apache-2.0` under the owner direction recorded in `provenance/SOURCE_GRANT.md` |
| local `artifact-binding` design package | working-copy identity `4e91c8868545` | Active Cairn proposal, design, specification, and tasks; no Rust crate or adoption claim | `MIT OR Apache-2.0` for workspace-owned artifacts |

These imports establish local source provenance only. They do not claim that the new `artifact` remote exists, that an immutable workspace revision has been published, or that any consumer has migrated.

## Semantic classification

| Semantic class | Cairn | Mantle | Molten | Valence | Canonical disposition |
| --- | --- | --- | --- | --- | --- |
| Version/domain/purpose | canonical JSON domain and evidence purpose | trust statement domain and suite | explicit domain ID/version and key purpose | protocol-owned Radicle entry domain | Shared typed statement fields; protocol codec remains consumer-owned. |
| Subject | recomputed receipt BLAKE3 | immutable OCI SHA-256 digest pair | canonical payload reference | signed entry/object identity plus Evidence IR artifact | One typed subject reference; digest algorithm remains explicit. |
| Parents/context | measured artifacts, policy export, Valence receipt | metadata manifest and local policy context | verifier-context reference | parent and related OIDs | Sorted unique typed parents plus one verifier-context reference. |
| Producer/key labels | producer plus key ID | signer labels | opaque handle purpose and public ref | Radicle actor key | Bound metadata only; labels never replace full-key identity. |
| Full-key identity | BLAKE3 of encoded public-key text | domain-separated BLAKE3 of full key material | public-key reference | protocol public key | Canonical BLAKE3 over raw Ed25519 public-key bytes. Consumer adapters map legacy identities explicitly. |
| Generation/currentness | revoked boolean | explicit revoked full-key list | generation plus current/overlap/superseded/revoked | no generic key-currentness contract | Explicit supplied generation/currentness observation and observation reference. |
| Threshold | one selected key | distinct verified full-key threshold; same-name keys retained | single verification outcome | no generic threshold | Distinct verified full-key identities only. |
| Cryptography | CLI Ed25519 verifier | Nix-format Ed25519 verifier | shell Ed25519 adapter | Radicle protocol verifier | Pure raw Ed25519 verifier package; protocol wrappers stay local. |
| Authorization | lifecycle policy | repository/release policy | capability/federation policy | exact actor-role policy | Retained by every consumer; never emitted by the shared core. |
| Measurement and canonical payload | Cairn CLI | OCI/build shells | Preserves/Iroh shell | Git/Radicle shell | Retained by every consumer. |

## Selected implementation route

Three mechanisms were evaluated:

| Family | Mechanism | Result |
| --- | --- | --- |
| Cairn-first | Generalize the existing typed observation evaluator. | Rejected alone: clean shell boundary, but product-specific receipt/Valence fields and no multi-key threshold. |
| Mantle-first | Move OCI or action-result trust code directly. | Rejected alone: strongest full-key threshold behavior, but OCI/Nix/build schemas and signing/key-management authority are product-specific. |
| Contract-first synthesis | Define a neutral typed statement and adapt the strongest pure invariants from Cairn, Mantle, and Molten. | Selected: preserves full-key threshold, purpose/currentness, deterministic issues, and product authority boundaries without a product dependency cycle. |

Valence is a conformance mapping, not implementation source for generic Ed25519. Its Radicle protocol owns the signed entry bytes and its Evidence IR/actor policy remains separate.

## Compatibility limits

- Cairn's historical public-key identity hashes lowercase hexadecimal key text. The standalone identity hashes raw public-key bytes. The adapter must dual-record both during migration; they are not silently equated.
- Mantle's Nix key identity hashes encoded key material under a Mantle domain. The adapter must retain that interoperability identity beside the standalone raw-key identity.
- Molten's overlap state can verify but cannot authorize new signing in the shared core. Currentness freshness remains a supplied consumer fact.
- Valence can map a protocol-verified actor key and signed-entry reference into an observation, but the shared core does not recreate or replace Radicle signature verification.
