# Interactive dashboard verification

## Outcome and ownership

The existing `--format html` surface is an offline interactive dashboard: case-insensitive run/active-attempt search, exact outcome filtering, deterministic sorting, bounded run and attempt drill-downs, and URL-fragment navigation. Text output, canonical state and provider authority are unchanged. The current consumer is the local Kiln operator; adoption uses the existing dashboard command. Kiln dashboard maintainers own the renderer, browser specification and bounded projection tests.

## Exercised runtime behavior

Generated actual HTML through both `kiln-dashboard` and `kiln dashboard`, using canonical journal admission and the checked-in dashboard profile. The synthetic browser journal contained six runs, seven attempts, successful retry history, failure, pending, unknown and cancelled outcomes, plus markup-bearing run and attempt IDs. Its admitted revision was 12. Also rendered and opened empty input, explicit missing targets, and the sample SQLite store.

Chromium interaction established:

- Searching `ATTEMPT-0-RETRY` showed only `run-0`; a missing query showed the empty state.
- Filtering `DomainFailure` showed only `run-1`; changing the filter retained its selected details with a hidden-row explanation.
- Descending numeric attempt count placed the two-attempt `run-0` first.
- Selecting the older `attempt-0` displayed its own `DomainFailure`, owning run and canonical view identity; reload and browser back restored the selected identity.
- Stale fragment targets displayed missing-target explanations with no substitute panels. An attempt belonging to another run displayed an ownership mismatch and no wrongly paired attempt panel.
- Markup-bearing identifiers stayed literal text: zero image elements, exactly the fixed interaction script, zero resource loads and no page errors.
- The page fit a 390-pixel viewport without document-wide horizontal overflow. Desktop light and narrow dark views were visually inspected.
- With JavaScript disabled, all six run panels and seven attempt panels remained readable, enhanced controls were hidden, and no external resource loaded.
- The public CLI's final snapshot selected `run-0` / `attempt-0-retry` and loaded zero external resources.

## Watch regression found and fixed

The first real watch smoke reproduced Chromium meta-refresh discarding a fragment changed after document load: revision advanced from 12 to 11, but search, outcome and selection reset. The renderer now embeds the admitted refresh interval for a fixed `location.reload()` timer, with the original meta-refresh inside `noscript` as fallback.

The corrected bounded watch run rewrote the report from revision 12 to 11. The browser retained search `run-1`, outcome `DomainFailure`, selected run `run-1`, selected attempt `attempt-1`, and fragment `#q=run-1&outcome=DomainFailure&sort=id&direction=asc&run=run-1&attempt=attempt-1`. The shell exited on the smoke's explicit timeout; no watch service remains running.

## Checks and scope

- `cargo test --locked -p kiln-dashboard`: 28 passed, including bounded detail parity, explicit selections outside the run page, attempt truncation plus active-attempt retention, redaction, empty/missing targets, deterministic bytes and admitted hostile-identifier escaping.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed. After the final run-identity iterator cleanup, affected dashboard/CLI Clippy passed again.
- `cargo fmt --check`: passed after final formatting.
- Proposal, design and task gates passed before implementation; the watch design was re-gated after the browser finding. Accepted HTML/live specs were synchronized without archiving or publication.
- Pinned Bombadil `832faf4d953e2c56d1d901aca1bb2c3c3170221d`: separate 30-second browser runs over the generated snapshot and watched document reached their time limits with no property violation. Properties exercised filtering, sorting, selection, mismatches, stale fragments, reloads and absence of external resources. Traces were retained at `/tmp/kiln-dashboard-bombadil-trace` and `/tmp/kiln-dashboard-bombadil-watch-trace`; these are local transient evidence, not published artifacts.
- `cargo test --locked --workspace` stopped at `kiln-adapter-github::tests::http_daemon_survives_more_than_concurrency_limit_and_rejects_ambiguous_headers`, `crates/kiln-adapter-github/src/tests.rs:718`, with observed 0 versus expected 1. No GitHub adapter source was changed or its failure rerun to obtain a green result.
- Cairn traceability reported no missing accepted requirements, but failed on six dangling `kiln.patchgate.*` markers. Release-readiness likewise failed its traceability check. The separate active Patchgate change was not modified.
- Final `nix flake check path:. --no-write-lock-file --builders '' -L` exited 1 at `checks.x86_64-linux.nix-eval-fixture-process`: `deterministic_fixture_processes_preserve_process_classes`, `crates/kiln-adapter-nix-eval/tests/fixture_process.rs:144`, did not observe the expected `OutputLimitExceeded` classification. That fixture reported two passing tests and one failure; Nix cancelled remaining builds. The Nix-evaluator adapter was not modified. This is a recorded repository-gate failure, not a passing flake check.

Initial verification attempts encountered a full home filesystem, compiler-cache I/O failures, and temporary-directory quota exhaustion. Relocating Nix cache writes and removing this task's duplicate compiler cache allowed focused tests, browser rails, Clippy and policy checks to proceed. The throwaway Rust journal generator was removed after use.

This initial evidence covers admitted local observations and the exercised browser sessions only. It does not qualify live CI, providers, deployment, source trust, completeness, newest-revision claims or release authority. No source publication, remote VCS mutation or lifecycle archival was performed.

## UI and DX follow-on

The local operator surface now uses a responsive console: navigation rail, compact admitted-state cards, outcome summaries, a run list beside the selected run/attempt inspector, and native disclosures for secondary evidence. Canonical view identities and truncation facts remain in selected panels. The same local Kiln operators are the consumer; Kiln dashboard maintainers own the renderer, fixed assets, preview example and documented edit loop. The durable contribution is a representative admitted preview and a repeatable source-to-browser workflow, not a one-off mockup.

### Exercised console behavior

- Opened the generated snapshot in Chromium at 1440×1000 and inspected light and dark presentations. At 390×844, the console stacked and the document width remained exactly 390 pixels, including with evidence expanded. Wide tables scrolled inside their surfaces.
- `/` focused run search. Searching `API-CHECKS-2` showed only `api-checks`. Typing `?` in search inserted the literal character without opening help; Escape cleared search. Outside editable controls, `?` opened the native dialog and focused its Close button; Escape closed it. The help button also worked at the narrow viewport.
- Exact `DomainFailure` filtering showed `integration`. Selecting that run, switching to dark theme and reloading preserved the filter, run and `integration-1` attempt. Invalid `theme=neon` fell back to system theme. Section navigation and the skip link changed focus without changing the selected-view fragment.
- Descending attempt-count sorting placed the two-attempt `api-checks` first. Selecting its older `api-checks-1` showed that attempt's `DomainFailure`, not the active retry's success, and retained the canonical view identity. Cross-run attempt selection displayed the ownership mismatch and no wrongly paired panel; missing targets displayed no substitute panels.
- With JavaScript disabled at 390 pixels, six run panels and seven attempt panels remained readable, zero enhanced controls appeared functional, the document did not overflow horizontally, and the native provenance disclosure exposed the synthetic journal schema.
- The inspected pages loaded zero external resources; the final managed browser reported no page errors. Managed inspection tabs were closed after use.

### Exercised developer workflow

- `cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-console` generated and canonically admitted the representative journal: six named runs, seven attempts and revision 13. It selected the successful `api-checks-2` retry, with the failed prior attempt still inspectable. No requested provider effect was executed.
- Rendering that generated journal with the actual `kiln-dashboard` CLI, the same profile and explicit selections produced byte-identical HTML (`cmp` exited zero).
- The documented `watchexec --restart` command ran inside the updated Nix dev shell with `preview ... --live`. A real CSS edit changed the desktop main padding from 32 to 24 pixels. The watcher rebuilt the example, atomically replaced the file and the browser observed 24 pixels after automatic reload, retaining query `integration`, exact outcome `DomainFailure`, run `integration`, attempt `integration-1`, dark theme and the complete fragment. The watcher was stopped after this proof.
- Preview `--help` succeeded. An unsupported option exited nonzero with usage before writing output.
- Copied the final snapshot to Aspen3's task-owned `/tmp/kiln-dashboard-vf0oGBqG.html`, opened it in the desktop account's LibreWolf, and confirmed the focused window was `Kiln dashboard dashboard-v1`.

### Follow-on checks and limits

- Fresh `cargo test --locked -p kiln-dashboard`: 28 passed.
- Fresh `cargo clippy --locked -p kiln-dashboard -p kiln-cli --all-targets -- -D warnings`: passed, including the preview example.
- `cargo fmt --check`: passed, including after the final trace annotations.
- `nix flake check path:. --no-build --no-write-lock-file --builders ''`: passed evaluation for x86_64-linux; this did not build every check and omitted incompatible aarch64-linux outputs.
- The initial browser rail stopped on its legacy literal-label `statesTheObservedFacts` assertion when no target was selected and secondary evidence was collapsed. That wording-only property, the corresponding label-only subassertions and unused full-page extraction were deleted rather than re-pinned to the new layout. Projection/identity parity remains checked by the existing Rust tests; actual disclosure readability and facts were exercised above. Behavioral filtering, sorting, exact selection, ownership mismatch, fragment restoration and resource-isolation properties were retained.
- The retained browser behavior rail then completed separate 30-second snapshot and automatically reloading-document runs without a property violation. The reloading run used the final specification after unused-extraction cleanup. Local traces: `/tmp/kiln-dashboard-console-behavior-trace-20261026` and `/tmp/kiln-dashboard-console-live-trace-20261026`; they are transient, scoped observations, not exhaustive exploration.
- Final Cairn traceability reported `missing: []` for 879 accepted requirements, and still failed solely on the six recorded dangling `kiln.patchgate.*` markers. CSS/JS requirements are anchored at their compiled Rust asset inclusions because the current trace profile does not scan those asset extensions. Marker coverage is declared linkage, not behavioral proof.
- The prior full-workspace GitHub test and full-build Nix fixture failures above were not rerun or claimed fixed by presentation work. No full-workspace or all-check build pass is claimed for this follow-on. No lifecycle archival, source publication or provider deployment occurred.

