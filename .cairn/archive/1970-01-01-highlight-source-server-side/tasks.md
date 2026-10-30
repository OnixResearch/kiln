## Phase 1: Implementation

- [x] [serial] Add the pure `kiln-highlight` crate with pinned Tree-sitter grammars, the fixed class vocabulary, UTF-16 spans, bounds and diff mapping. r[kiln.highlight.core] r[kiln.highlight.core.scenario.offsets] r[kiln.highlight.core.scenario.skipped]
- [x] [serial] Add opt-in `highlight=1` to the Source file and diff reads and the code-review change detail. r[kiln.highlight.api]
- [x] [serial] Render server tokens in the Source file view, the Changes view and the code-review diff, remove the client tokenizer, and add theme tokens. r[kiln.highlight.render]

## Phase 2: Verification

- [x] [serial] Run Rust and node tests, a real-data browser review, the repository gates, record binary size and licenses, deploy and retain evidence. r[kiln.highlight.core] r[kiln.highlight.api] r[kiln.highlight.render]

Evidence: `evidence/verification.json` and four viewport screenshots. Accepted on operator request 2026-10-30.
