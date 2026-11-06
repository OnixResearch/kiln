## Context

Core review tests index fixture vectors directly. MicroVM tests repeat generated command/JSON fields instead of exercising guest behavior. The isolated Octet consumer copies the real Defelo source; its explicit-defaults finding originates in `serde(default)` on an `Option<String>`.

## Decisions

Use `first_mut().required()?` for meaningful review-test mutations, preserving failure when fixtures are malformed. Delete implementation-wiring microVM tests instead of changing their assertions to another spelling. Existing real Firecracker cache and native confinement journeys remain the behavioral evidence.

The first repaired Clippy run exposed a Lattice test that only copies the lowered review-node kind and length. Delete that implementation-wiring test too; retain the adapter's meaningful admission, failure and protocol conformance tests. Production Lattice code and contract pins stay unchanged.

Later compilation exposes Radicle helper mock-output assertions and the review outcome classifier. Remove output-copy checks, retaining invalid-verdict refusal before child effects and helper-failure coverage. In `thread_outcome`, both indices come from positions in the same immutable revision vector, and earlier guards establish `anchor_index < resolved_index`. Iterate with `skip(anchor_index + 1).take(resolved_index - anchor_index)` to select exactly the previous inclusive slice without panicking indexing, allocation, fallback or changed comparison limits. Elide `side_path`'s redundant lifetime and use existing `first().ok_or(...)?` fixture access in outcome tests.

Apply the same rule to further test-only indexing findings exposed by full compilation: retain exact revision, trust, signed-source and authority-forgery assertions, accessing JSON with `pointer`, `pointer_mut`, or explicit object insertion. Do not delete authority checks or change production admission to clear a fixture lint.

Serde already decodes an absent optional field as `None`. Remove only `default`; retain `skip_serializing_if = "Option::is_none"`. Freeze omitted/null/string decoding and exact serialization with positive and negative contract cases. Do not introduce a custom default, suppression, replacement protocol or altered dependency pin.

## Risks / Trade-offs

Derive-generated code changes despite unchanged intended wire behavior. Exercise the contract before and after the attribute edit, including malformed IDs and missing-ID trigger refusal. Passing lint gates is not release eligibility or host qualification. Preserve all unrelated active lifecycle work.
