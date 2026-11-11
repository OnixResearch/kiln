## Context

`cargo-fuzz` requires nightly sanitizers and a separate crate. Keeping the fuzz
crates outside the workspace avoids adding nightly and libFuzzer to normal
builds. The web-service feed parser accepts external RSS/RDF/Atom and uploaded
OPML; the highlighter feeds repository content to pinned Tree-sitter grammars.

## Decisions

### Separate fuzz crates

`crates/kiln-web-service/fuzz` and `crates/kiln-highlight/fuzz` each have their
own `[workspace]` marker and are listed in the root `workspace.exclude`. Their
`Cargo.lock`, `target/` and `artifacts/` are ignored. Seed corpora are checked
in.

### Shrunk web-service limits

The `kiln-web-service` `fuzzing` feature lowers the absolute feed byte bound,
OPML outlines, feed depth and text bounds. The fuzz crate enables this feature;
normal builds keep the production constants.

### XML control characters

`escape` previously emitted C0 controls such as U+0001 into Atom/RSS/OPML,
producing XML 1.0 that readers reject. It now omits every control except tab,
newline and carriage return, matching `clean_text`'s input policy. A regression
test renders and reparses all three formats.

### Cover both bound configurations

The highlight target selects `Bounds::DEFAULT` or a small `Bounds`; the
structural target does the same with `StructuralBounds`. The web-service target
chooses `max_entries`/`max_outlines` from the input. This keeps truncation and
fallback branches reachable.

## Risks

- ASan/UBSan coverage depends on how the `cc` crate builds grammar C sources.
  The documented `CFLAGS` are required for C-side UBSan.
- A clean smoke run does not prove grammar memory safety; longer campaigns and
  upstream fixes remain separate.
- Fuzz crates are not built by `cargo check --workspace`; their own `cargo
  check` and smoke runs are separate evidence.
