# Structural preview evidence verification

Issue: <https://github.com/OnixResearch/kiln/issues/65>, optional slice from the
[17 November comment](https://github.com/OnixResearch/kiln/issues/65#issuecomment-6956282283).

## Delivery and ownership

The immediate result is an opt-in `kiln.preview.structural.v1` record on
`GET .../preview-comparison?structural=1` and the matching section of the review
comparison view. The durable capability is a reusable binding between the
review's structural engine and exact preview subjects. The pieces are a
lock-checked grammar description in `kiln-highlight` (`STRUCTURAL_RULES`,
`GRAMMAR_PINS`, `grammar_set()`), one shared per-file structural function for
both the review diff and preview evidence, and a frozen fixture that holds both
paths to the same output. Kiln review/web and preview maintainers own it. The
current consumer is the #65 comparison view. The graphical cohort in the
issue's earlier comment can attach later evidence the same way.

## Exercised checks

Commands ran from this checkout through `nix develop path:$PWD -c`.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal|design|tasks add-structural-preview-evidence ...` | PASS before implementation; task gate PASS again after the tasks were ticked |
| `cargo test --locked -p kiln-highlight` | 33 passed, including the 4 pin checks |
| `cargo test --locked -p kiln-web-service --lib` | 269 passed, 2 ignored |
| `cargo test --locked -p kiln-web-service --test code_review_interdiff` | 6 passed; structural mode is unchanged after the shared-function refactor |
| `cargo clippy --locked -p kiln-highlight -p kiln-web-service --all-targets -- -D warnings` | PASS after two test assertions stopped indexing JSON values |
| `cargo test --locked -p kiln-web-service --lib -- structural` after that fix and formatting | 7 passed |
| Scoped `rustfmt --check --edition 2024 --config skip_children=true` | PASS for the 7 touched Rust files |
| `node --test tests/preview-compare.test.mjs tests/artifact-preview.test.mjs tests/console-inventory.test.mjs tests/live-console-ui.test.mjs` | 90 passed |
| Disposable browser layout check (below) | Opt-in, keyboard, wide and narrow layouts observed |

Rust coverage, positive and negative:

- The pin checks require each pin to match exactly one `Cargo.lock` entry by
  package, version and checksum. Stale versions, stale checksums, unlocked
  packages, duplicate entries and checksum-less entries are refused. The pinned
  set must equal the crate's `tree-sitter*` dependencies, and every language
  must map to a pinned grammar.
- Fake-provider tests cover the following:
  - An incompatible comparison yields `incompatible` with its reasons and no
    files, and the provider is never read. A compatible pair reads it once.
  - `unsupported_provider` and `source_unavailable` arise from comparison and
    blob failures and stay inside the record.
  - Each file fallback is named: `binary`, `truncated` (per file and per
    comparison), `too_large`, `unsupported_provider`, `unsupported_language`
    and `parse_error`. No fallback carries rows.
  - The record identity is recomputable from the returned body and changes with
    the comparison, the commit order, the bounds, an `interrupted` outcome and
    the status.
  - A record over 2 MiB keeps no files and reports `evidence_exhausted`. One
    within the bound keeps all 1,024 files.
- The route tests drive the authenticated review shell over a real Git
  repository built from the frozen fixture, with two retained compatible
  observations and one observation that has different program arguments:
  - Without `structural=1`, the response equals the structural response minus
    its record.
  - The record binds the comparison identity and both exact commits.
  - Its files equal `revision=<candidate>&base=<baseline>&mode=structural` and
    the frozen `expected.json`. Its settings equal the frozen `settings.json`.
  - The reformat shows no change, the function moves from lines 9–11 to 1–3,
    the edit has one novel token per side, and `broken.rs`, `notes.txt` and
    `pixel.bin` report `parse_error`, `unsupported_language` and `binary`.
  - `structural=0`, `yes` and an empty value return
    `400 preview_comparison_query_invalid`. Unlike arguments yield
    `incompatible`, a historical candidate is still refused with 409, and the
    change detail is byte-identical before and after.

The Node tests serve the frozen fixture's real output to the renderer. They
check that the record applies only to its comparison and both revisions, that
fallbacks and unknown reasons are named, that rows are bounded with exact line
numbers, and that nothing is requested or drawn until the opt-in is selected.
Selecting it routes the same pair with the opt-in. Incompatible, unavailable and
mismatched records are explained without substitution.

## Browser check

A disposable local Bun server served the real `preview-review.js`,
`code-review.css`, `web.css`, `identity.css` and `catalog.css`, plus the frozen
fixture's `expected.json` and `settings.json`, to managed headless Chromium. The
page rendered the comparison with two synthetic observations. It is not the
authenticated console and launches no artifact.

- 1280×900: the lanes are side by side and the checkbox is unchecked. The only
  request carried `structural=false`, and no structural section was drawn.
  Using only the keyboard, Space ticked the checkbox and Enter on **Compare**
  routed `?structural=1`. That issued one request with `structural=true`,
  drew one structural section below both lanes and moved focus to the
  comparison heading. The page had no horizontal overflow.
- 390×844, restored from `?structural=1`: the checkbox stayed checked, the lanes
  stacked into one 340 px column, the structural section stayed within the
  viewport and the page had no horizontal overflow.

The server was stopped and the tab released afterwards.

## Deliberate choices

- Records are computed on request and not retained. The retained observations
  and the provider's exact commits already pin the inputs, and a second store
  would add expiry and cleanup without adding fidelity.
- The comparison uses `DiffLimits::default()` and exact whitespace, as
  `base=<revision>` does, with no forced note. Fidelity follows from the shared
  per-file function; the fixture test enforces it.
- `kiln-highlight` exports the grammar description but no hash function, so its
  linked registry set remains the one recorded in `LICENSES.md`. The review
  service hashes the description under `kiln.highlight.grammar-set.v1`.
- Provider failures are recorded in the evidence lane rather than returned as
  route errors, so a failed lane leaves the comparison readable.

## Boundaries

- Fidelity is proven for the frozen fixture's six files only. The #10 OmniDiff
  corpus was not run and supplies no target here.
- Neither `STRUCTURAL_RULES` nor `kiln.preview.structural-rules.v1` changes on
  its own. A change to the engine's behavior is caught only where the frozen
  fixture exercises it.
- No live preview cohort, TryNix launch or authenticated-console browser run was
  performed for this slice. The graphical cohort in the issue's earlier comment
  remains open.
- No review-time, reviewer-throughput or merge-speed claim is made.
- The full workspace tests, Octet, release-readiness and flake checks were not
  run.

## Lifecycle

Native `cairn sync add-structural-preview-evidence ... --execute` added
`kiln.preview.structural` and its three scenarios to
`.cairn/specs/artifact-preview/spec.md`; nothing was archived. `cairn validate`
passed. Repository-wide traceability coverage still exits 1 with the same 71
unrelated dangling IDs and the two missing GitHub anchors
(`kiln.github.landing.merge.scenario.landed`,
`kiln.github.source_status.scenario.close`). Every `kiln.preview.structural` ID
is referenced.
