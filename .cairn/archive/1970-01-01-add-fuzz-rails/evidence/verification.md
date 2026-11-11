# Verification evidence

## Result

The out-of-workspace fuzz targets are implemented and smoke-run. Feed and OPML
targets parse arbitrary bytes under shrunk bounds; Atom/RSS/OPML render-parse
round-trips and URL admission are covered. Highlight targets cover content
highlighting, unified-diff highlighting and structural diffing under small and
default bounds, with ASan on Rust and C grammar code plus C-side UBSan.

XML rendering now strips C0 controls that XML 1.0 forbids; a regression test
renders and reparses Atom, RSS and OPML.

## Smoke runs

- `feed_parse`: 36,213 runs, 0 findings.
- `opml_parse`: 26,903 runs, 0 findings.
- `atom_roundtrip`: 15,644 runs, 0 findings.
- `opml_roundtrip`: 20,919 runs, 0 findings.
- `rss_roundtrip`: 17,567 runs, 0 findings.
- `admit_url`: 25,662 runs, 0 findings.
- `highlight`: 19,136 runs, 0 findings.
- `highlight_diff`: 121,038 runs, 0 findings.
- `structural_diff`: 26,218 runs, 0 findings.

The ASan runs used `cargo-fuzz` with a rustup nightly and the documented
OpenSSL/libstdc++ library paths. The C grammar sources were built with
`CFLAGS=-fsanitize=undefined -fno-sanitize-recover=all`.

## Non-claims

These are short smoke runs, not exhaustive fuzzing. A clean run does not prove
parser correctness or Tree-sitter grammar memory safety, and does not create a
service-confinement or release claim. Any future crash belongs in the owning
crate's ordinary regression tests, with an upstream grammar report or pin.
