# Verification

Issue [#41](https://github.com/OnixResearch/kiln/issues/41).

`crates/kiln-core/src/interchange.rs` now maps one admitted, attempt-validated
`ProvenanceProjection` to canonical unsigned JSON. Object keys are ordered and
output is compact; products and native digests are sorted and deduplicated, so
equal projections produce equal bytes.

The golden fixture `crates/kiln-core/testdata/slsa-provenance-statement.json`
pins the exact statement. Tests cover the golden bytes, refusal of a projection
bound to another attempt, order independence, an empty subject set, colliding
native digest keys, and separation of builder, invocation, provider, worker and
control-plane identities.

The run Evidence view recognizes an in-toto Statement under the existing
`provenance` report kind and renders subjects, dependencies and identities with
an explicit non-claim; `node --check` passed for the asset.

Unsigned by design. No DSSE envelope, signature, SLSA build level, source trust
or release authority is claimed.
