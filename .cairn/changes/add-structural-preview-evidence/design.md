## Context

`preview::observation::compare` derives `kiln.preview.comparison.v1` from two retained, projected observations. The `/preview-comparison` route in `code_review.rs` serves that comparison and never substitutes a subject. The candidate must be current; the baseline may be historical, and each binding names an exact revision commit of the change. The review's structural mode (`code_review/structural.rs::files`) diffs every file of one provider `CommitComparison` with `kiln_highlight::structural_diff`. It applies the default bounds and a two-second deadline per response, and every file gets either structural rows or a named fallback. The diff route uses it for `base=<earlier revision>&mode=structural`, which compares the two exact revision commits.

## Goals and Success Contract

A reviewer who opts in sees, next to the two observations, the syntax-aware difference between the baseline and candidate source, plus the exact identities and settings that produced it. For the same commits, it is the result the review's structural mode shows. When the evidence is withheld or a file falls back, a named reason explains why. With the option off, the comparison is unchanged.

## Decisions

### Decision: reuse the review's structural function

**Choice:** the route compares `baseline.binding.revision` with `candidate.binding.revision` through the review provider. It uses `DiffLimits::default()` and exact whitespace, as `base=<revision>` does, and passes the result to the same per-file function as structural mode, with no note. The per-file loop moves into a shared function. `files` keeps its response shape.

**Rationale:** fidelity follows from the shared code path; a second engine or a separate bounds table would drift. The frozen fixture checks that both routes produce the same result.

### Decision: lock-checked grammar description in `kiln-highlight`

**Choice:** `kiln-highlight` exports `STRUCTURAL_RULES`, `GRAMMAR_PINS` and `grammar_set()`. The pins cover the parser runtime, `tree-sitter-highlight` (its query configuration decides whether a grammar is available) and every grammar. Each pin records the package, version and Cargo registry checksum, and the set records which package parses each language. Unit tests require every pin to match exactly one `Cargo.lock` entry and require the pinned packages to equal the crate's `tree-sitter*` dependencies. The review service hashes the serialized set under its own domain as `grammar_identity`. `kiln-highlight` links no hash function, so its linked registry set stays as recorded in the license map.

**Rationale:** grammar names alone do not identify grammar code. The lock file's checksum does, and a test keeps the description from going stale.

### Decision: explicit opt-in, off by default

**Choice:** the route accepts `structural=1` and refuses any other value with `400 preview_comparison_query_invalid`. When the parameter is absent, the route performs no provider or grammar work and returns the existing body. The review page adds an unchecked checkbox to the comparison form and restores it from `review_preview_structural=1`.

### Decision: incompatible subjects withhold structural evidence

**Choice:** when the comparison is incompatible (same observation, or a different engine, platform, argument list or executable), the record has status `incompatible` with the comparison's reasons and no files. The provider is not read.

**Rationale:** the issue forbids comparing unlike runtime cohorts as if only the source changed.

### Decision: record computed on request, bounded and self-identifying

**Choice:** the record is computed for each request and is not stored. The retained observations remain the stored evidence, and the provider keeps the exact commits. The body binds the schema, comparison identity, both commits, settings, status, reasons, files and fixed provenance. Its identity is the BLAKE3 hash of a domain tag and the serialized body. Statuses are `structural`, `incompatible` and `unavailable`. Lane-local provider failures stay in the record: `unsupported_provider` when the provider cannot compare, and `source_unavailable` for any other comparison or blob error. Neither failure becomes a route error. A serialized body over 2 MiB is replaced by `unavailable` with `evidence_exhausted` and no files, so the 4 MiB response bound still holds. An interrupted file reports `interrupted`. Its record identity therefore differs from an uninterrupted run, so it cannot pass for that result.

**Rationale:** a persistent store would add retention, expiry and cleanup with no extra fidelity. Keeping failures inside the record leaves the comparison intact when the lane cannot run.

### Decision: frozen fixture with settings and expected outcomes

**Choice:** `crates/kiln-web-service/tests/fixtures/structural-preview/` contains Kiln-authored `baseline/` and `candidate/` trees, `settings.json` and `expected.json`. The trees hold a pure Rust reformat, a moved unchanged function, one literal edit, a parse error, a file without a grammar and a binary file. The route-level test builds the commits in a disposable repository, retains two compatible observations and requests both the preview record and `revision=<candidate>&base=<baseline>&mode=structural`. It requires equal files, frozen files and frozen settings. A deliberate engine, bound or pin change therefore fails until the fixture is refreshed and the rules version is reviewed.

## Risks / Trade-offs

- A change to the structural engine that leaves the rules version unchanged would keep its identity. Only the frozen fixture detects that drift.
- The two-second deadline can interrupt a large comparison. The record then names `interrupted`, so a timed-out file is reported as such rather than as an empty diff.
- The record describes source structure only. It says nothing about runtime behavior, review speed or correctness.

## Verification Strategy

`kiln-highlight` unit tests check the lock, the manifest and the language mapping, with negative matcher cases. In-crate fake-provider tests cover incompatible, unsupported, unavailable, binary, truncated, oversized and size-bound records and their identity bindings. The route-level fixture test proves equality with structural mode, the frozen outcomes and settings, off-by-default behavior, query refusal and unchanged review state. Node tests cover the opt-in control, binding checks, fallback text and bounded rendering. A disposable browser page loads the served `preview-review.js` and `code-review.css` with fixture-derived data and records wide and narrow layouts and keyboard operation.
