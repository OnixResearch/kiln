# Verification evidence

## Result

Kiln now distinguishes Mantle's logical store prefix from the physical output
and state roots, admits the complete top-level `crunch-build-report-v1` surface
serialized by the pinned cohort, rejects shape and count drift, binds failure
logs, and records deterministic native collection and cache metrics. Focused
tests, workspace compilation, Clippy, formatting, Nickel profile checks and
Cairn validation pass.

## Evidence

- `cargo test -p kiln-adapter-mantle` passes 19 tests. The suite covers a
  complete report with every native collection, distinct logical and physical
  roots, unknown and missing top-level fields, count/array disagreement, FOD
  mismatch without failure, path escapes, unknown failure vocabulary, failure
  log readability, substitution byte/fallback metrics, and a captured native
  failure report.
- The captured report was produced by a real Mantle invocation:
  `mantle --store <temp-store> --state-dir <temp-state> --json build
  examples/hello.ncl --no-substitute`. The local producer reached the sandbox
  failure path and emitted the full `crunch-build-report-v1` document; the
  fixture preserves that shape with profile paths substituted.
- `cargo clippy -p kiln-adapter-mantle --all-targets -- -D warnings` passes.
- `cargo check --workspace --all-targets` passes.
- Workspace tests, rustfmt, Nickel profile export and negative fixtures, and
  Cairn validation are recorded in `verification.json`.
- `cairn validate --root .` reports no issues, change issues, spec issues, or
  findings.

## Non-claims

Native collection metrics describe admitted report shape and counts. They do
not prove build correctness, sandbox completeness, cache truth, product
semantics, reproducibility or release readiness. Lexical root checks do not
prove filesystem containment, symlink safety or source trust. The captured
fixture is report-shape evidence; it is not an executable identity or cohort
refresh.

Nested Mantle evidence remains bound by the exact report receipt. Kiln does not
reinterpret it as product, cache, CI or release authority.
