## Why

Kiln pins SLSA Provenance `v1.1` and in-toto Statement `v1` as reference vocabularies and defines a typed `ProvenanceProjection`, but no code path serialized either document. Consumers that verify provenance could not read anything Kiln produced even though the identity roles and digests were already admitted.

Issue: [#41](https://github.com/OnixResearch/kiln/issues/41).

## What Changes

- Add `slsa_provenance_statement`, a pure mapping from one admitted, attempt-validated `ProvenanceProjection` to canonical unsigned JSON: an in-toto Statement `v1` whose predicate type is `https://slsa.dev/provenance/v1.1` and whose build type is `urn:kiln:provenance:ci-run:v1`.
- Emit each product as a subject with its BLAKE3 digest; resolve the source or candidate as one dependency and each provider-native digest as a separate dependency with a Kiln-scoped identity-kind key.
- Keep builder, invocation (producing attempt), provider, worker and Kiln control-plane identities in distinct fields.
- Sort and deduplicate products and native digests before encoding, so equal admitted projections produce equal bytes.
- Refuse a projection bound to another attempt, an empty product set, and colliding native digest keys before serialization.
- Retain statements under the existing `provenance` report kind and render subjects, dependencies and identities in the run Evidence view with an explicit non-claim.

## Impact

- **Files**: `crates/kiln-core/src/interchange.rs`,
  `crates/kiln-core/testdata/slsa-provenance-statement.json`,
  `crates/kiln-web-service/src/web.js`, `docs/integrations.md`.
- **Testing**: canonical golden-file test, attempt-mismatch refusal, order
  independence, empty subjects, digest-key collision, distinct role fields,
  `node --check` for the UI asset, and the core/web-service suites.
- **Authority**: the statement is unsigned identity evidence. It claims no SLSA
  build level and grants no builder, signer, source-trust, gate, merge or
  release authority. DSSE signing with an operator key remains separate work.
