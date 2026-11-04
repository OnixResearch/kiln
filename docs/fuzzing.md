# Fuzzing

Kiln keeps fuzz targets outside the Cargo workspace so a normal build does not
require a nightly sanitizer toolchain. The targets are in
`crates/kiln-web-service/fuzz` and `crates/kiln-highlight/fuzz`.

## Targets

`kiln-web-service`:

| Target | Entry point |
| --- | --- |
| `feed_parse` | `feeds::parse(bytes, max_entries)` |
| `opml_parse` | `feeds::parse_opml(bytes, max_outlines)` |
| `atom_roundtrip` | `feeds::render_atom` → `feeds::parse` |
| `opml_roundtrip` | `feeds::render_opml` → `feeds::parse_opml` |
| `rss_roundtrip` | `feeds::render_rss` → `feeds::parse` |
| `admit_url` | `feeds::admit_url` and `feeds::globally_routable` |

`kiln-highlight`:

| Target | Entry point |
| --- | --- |
| `highlight` | `highlight`, `language_for_path` |
| `highlight_diff` | `highlight_diff` |
| `structural_diff` | `structural_diff` |

The web-service fuzz crate enables the `fuzzing` feature, which shrinks the
feed, OPML, depth and text bounds so truncation and rejection branches are
reachable. The highlight targets select both a small `Bounds` and
`Bounds::DEFAULT`.

## Running

The toolchain is intentionally not part of the default dev shell. Use a
nightly-capable libFuzzer toolchain (for example `rustup` with a nightly
toolchain) and the fuzz crate directly:

```console
nix shell nixpkgs#cargo-fuzz nixpkgs#rustup nixpkgs#pkg-config \
  nixpkgs#openssl nixpkgs#gcc -c bash
export OPENSSL_INCLUDE_DIR=/nix/store/...-openssl-3.5.9-dev/include
export OPENSSL_LIB_DIR=/nix/store/...-openssl-3.5.9/lib
export PKG_CONFIG_PATH=/nix/store/...-openssl-3.5.9-dev/lib/pkgconfig
export LD_LIBRARY_PATH=/nix/store/...-openssl-3.5.9/lib:/nix/store/...-gcc-16.2.0-lib/lib
export ASAN_OPTIONS=abort_on_error=1:halt_on_error=1:detect_stack_use_after_return=1
export UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1
cd crates/kiln-web-service/fuzz
cargo +nightly fuzz run feed_parse -- -max_total_time=45 -rss_limit_mb=1024 -timeout=5
```

For the highlight C scanners, build with ASan (cargo-fuzz default) and pass the
C UBSan flags through `CFLAGS`:

```console
export CFLAGS="-fsanitize=undefined -fno-sanitize-recover=all"
cd crates/kiln-highlight/fuzz
cargo +nightly fuzz run highlight -- -max_total_time=60 -rss_limit_mb=1024 -timeout=5
cargo +nightly fuzz run highlight_diff -- -max_total_time=30 -rss_limit_mb=1024 -timeout=5
cargo +nightly fuzz run structural_diff -- -max_total_time=30 -rss_limit_mb=1024 -timeout=5
```

Seed corpora are checked in under each crate's `fuzz/corpus/`. Minimized
crashers belong in the owning crate's ordinary `#[test]` module, not in the fuzz
crate, so normal `cargo test` protects them.

## Boundaries

A fuzz pass is local robustness evidence, not a proof of parser correctness,
grammar memory safety, service confinement or release eligibility. Any crash in
a Tree-sitter grammar's C scanner must be reported upstream with the exact
grammar crate and version, or pinned/patched here with a regression.
