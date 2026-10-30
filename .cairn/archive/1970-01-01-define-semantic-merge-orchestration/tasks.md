## Phase 1: Contract

- [x] [serial] Write `docs/semantic-merge-orchestration.md`. It covers the state-transition contract, the integration plan with existing provider ports, Pijul provenance versus content identity, and the explicit non-claims. r[kiln.semantic_merge.request] r[kiln.semantic_merge.lineage] r[kiln.semantic_merge.dispatch] r[kiln.semantic_merge.independence]

## Phase 2: Pure core and port

- [x] [serial] Add `kiln_core::semantic_merge` with the immutable four-role request, role-tagged BLAKE3 binding, explicit lineage, and authorized candidate acquisition. r[kiln.semantic_merge.request] r[kiln.semantic_merge.lineage] r[kiln.semantic_merge.candidate]
- [x] [serial] Add the outcome lattice and modes. Add the event-sourced attempt state machine with result, artifact, coverage, cache and retry admission, plus freshness, gate combination and the revalidation obligation. r[kiln.semantic_merge.outcomes] r[kiln.semantic_merge.binding] r[kiln.semantic_merge.freshness] r[kiln.semantic_merge.independence]
- [x] [serial] Add the Lattice workflow plan mapping and the `SemanticCheckerPort` in `kiln-provider-api`. r[kiln.semantic_merge.dispatch]

## Phase 3: Fixtures and verification

- [x] [serial] Author the frozen quartet and negative fixtures in Nickel. Export them deterministically and add the `semantic-merge-fixtures` flake check. r[kiln.semantic_merge.dispatch.scenario.e2e]
- [x] [serial] Add the end-to-end fixture test through the Pijul adapter mapping and a fixture checker producer. Add the negative fixture tests and the forbidden-capability refusal test. r[kiln.semantic_merge.dispatch] r[kiln.semantic_merge.candidate.scenario.adapter] r[kiln.semantic_merge.binding] r[kiln.semantic_merge.freshness]
- [x] [serial] Run the focused fmt, clippy and tests, and record `evidence/verification.json`. r[kiln.semantic_merge.request] r[kiln.semantic_merge.outcomes] r[kiln.semantic_merge.independence]

Evidence: `evidence/verification.json`. Production gate deferred to Octet #1, Noble #30 and Valence #1 as stated in issue #1.

Accepted on operator request 2026-10-30 after parent integration review; see `evidence/verification.json`.
