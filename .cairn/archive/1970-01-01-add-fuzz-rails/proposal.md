## Why

Two parser/renderer surfaces take arbitrary external bytes: repository feed and
OPML handling in `kiln-web-service`, and Tree-sitter highlighting plus
structural diffing in `kiln-highlight`. Existing unit tests cover intended
cases but not arbitrary input. The workspace forbids panics, and the highlight
crate links C grammar scanners whose memory bugs would not appear as Rust
panics.

Issues: [#29](https://github.com/OnixResearch/kiln/issues/29),
[#30](https://github.com/OnixResearch/kiln/issues/30).

## What Changes

- Add an out-of-workspace `cargo-fuzz` crate for feed/OPML parsing, Atom/RSS/OPML
  rendering round-trips, URL admission, and shrunk limits under a `fuzzing`
  feature.
- Fix XML rendering to strip C0 controls that XML 1.0 forbids.
- Add an out-of-workspace `cargo-fuzz` crate for `highlight`, `highlight_diff`
  and `structural_diff`, with small and default bounds.
- Add seed corpora and a documented sanitizer/smoke procedure.

## Impact

- **Files**: `crates/kiln-web-service/fuzz`, `crates/kiln-highlight/fuzz`,
  `feeds.rs` bounds/escaping, `docs/fuzzing.md`, workspace excludes.
- **Testing**: normal crate tests plus the smoke fuzz runs recorded in evidence.
- **Authority**: fuzzing is local robustness evidence, not parser correctness,
  grammar memory-safety proof, confinement or release eligibility.
