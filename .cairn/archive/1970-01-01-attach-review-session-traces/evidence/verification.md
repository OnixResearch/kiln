# Session-trace identity verification

Issue: <https://github.com/OnixResearch/kiln/issues/91>.

## Delivery and ownership

The immediate result is default-off, exact-revision local Git-note pointer
capture and display in Kiln code review. The durable capability is a pure
bounded pointer-identity contract, an optional read-only provider port and
immutable observations in the existing revision journal. Kiln maintainers own
these core, Git and web surfaces; the operator owns profile opt-in and admission
of non-sensitive pointer IDs. Capture and session-body normalization remain
outside Kiln. No traces.com runtime, CLI or hosting dependency was added.

## Exercised checks

Commands ran from this checkout; dependency-bearing Rust and Nickel commands
used `nix develop path:$PWD -c`. Proposal, design and task gates passed before
implementation. The delta was subsequently aligned with the existing
`change-review` capability, and all three gates passed again.

| Check | Observed result |
| --- | --- |
| Baseline `cargo test --locked -p onix-kiln-core change_review` | 61 passed before implementation |
| `cargo test --offline -p onix-kiln-core change_review` | 65 passed; Cargo refreshed the existing lock graph for the new direct URL dependency |
| `cargo test --locked -p onix-kiln-core identities_frame_commit_digest_id_and_host_without_source_body_claims` | 1 passed after adding explicit forged-host refusal coverage |
| `cargo test --locked -p kiln-adapter-git --lib session_traces` | 2 passed |
| `cargo test --locked -p kiln-adapter-git --test change_review_session_traces` | 7 passed, with real local notes and fanout trees |
| `cargo test --locked -p kiln-web-service --lib code_review` | 41 passed, including disabled reads/bytes, immutable capture, restart and ambiguous recovery |
| `cargo test --locked -p kiln-web-service --test code_review` | 27 passed, including real provider effects and independent process writers |
| `cargo test --locked -p kiln-implementer native_publication` | 8 matching tests passed; other integration binaries had zero selected tests |
| `node --test tests/live-console-ui.test.mjs` | 69 existing tests passed; the new test initially exposed an incomplete test-DOM bootstrap |
| `node --test --test-name-pattern='session pointers' tests/live-console-ui.test.mjs` | 1 passed after initializing the real UI dependency and empty document query surface |
| `scripts/check-nickel-fixtures.sh` | 91 positive and 616 negative fixtures passed, with typechecking and formatting |
| `cargo check --locked -p onix-kiln-core --target wasm32-unknown-unknown` | PASS, pure no-std contract |
| `cargo clippy --locked -p onix-kiln-core -p kiln-provider-api -p kiln-adapter-git -p kiln-implementer -p kiln-web-service --all-targets -- -D warnings` | PASS |
| Scoped `rustfmt --check --edition 2024 --config skip_children=true` | PASS for all 17 changed Rust files |
| Native `cairn sync attach-review-session-traces ... --execute` | One additive requirement merged into `.cairn/specs/change-review/spec.md`; no archive |
| Pre-commit `cargo fmt --all --check` | PASS |
| Pre-commit `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS for every workspace member and target |
| Pre-commit `cargo clippy --locked -p kiln-web-service --all-targets --all-features -- -D warnings` | PASS; flake-gate feature set for the only changed crate declaring features |
| Pre-commit `cargo test --locked -p onix-kiln-core -p kiln-provider-api -p kiln-adapter-git -p kiln-implementer -p kiln-web-service` | 944 passed, 0 failed; 8 existing environment-gated tests ignored |
| Pre-commit `node --test` over all four `tests/*.test.mjs` files | 86 passed |
| Pre-commit `cairn validate ...` and `cairn gate tasks attach-review-session-traces ...` | PASS; 5 of 5 tasks complete |

The host's existing kache index reported an I/O error and disabled caching;
compilation and the listed checks succeeded without cache hits/stores. No cache
configuration or compiler contract was changed.

## Actual review-surface rehearsal

[`browser.json`](browser.json) retains the bounded native Git/revision
observations and browser assertions. A temporary seed in the existing `explore`
example enabled capture only in its disposable profile and created three exact
commits/revisions: two admitted pointers, no note, and an incompatible note. An
operator-declared synthetic agent supplied independent authorship and submission
metadata. No operated profile, repository or service state was changed.

The real page displayed the selected historical revision's IDs, BLAKE3 pointer
and note identities, source kind and normalized host. Selecting revisions 2 and
3 showed absence and malformed-note unavailability respectively, never stale
revision-1 IDs. Declared authorship and attested submission were separately
visible. The card had no links, active elements or retained URL path/query; the
page had no horizontal overflow at 1440 CSS pixels. Required CI and submit
readiness remained false. All 192 requests in the captured network sample were
to loopback; no external trace request was observed.

The current binary was then rebuilt with the original, absent-option example
profile. It returned no revision field and rendered no trace panel. Its 214
heading/label/summary/button/select entries matched the existing pre-feature
binary after normalizing volatile identities and the one disposable source
path. This is a bounded control-surface comparison, not a whole-page byte hash.
Permanent fixtures separately check exact legacy revision serialization and
unchanged durable document bytes. The temporary example instrumentation was
removed; its file snapshot returned to its original identity.

## Boundaries

- The admitted grammar is `traces:<externalId> [https-shared-url]`, read from
  exact local `refs/notes/traces` only. Other formats are explicit incompatible
  observations. Provider failures never masquerade as absent notes.
- BLAKE3 identities bind commit, raw pointer-note digest, source kind, declared
  ID and retained host. They are not hashes or attestations of unseen session
  bodies, and do not prove that a model ran.
- The default is no capture. Enabled ID/host text is retained literally under
  named bounds; operators must not encode secrets in those identifiers. URL
  authority credentials are rejected, paths/queries/fragments are not retained,
  and no raw session body is copied or rendered.
- Trace presence never satisfies CI, ownership evidence, intent approval,
  approval counts, checklist readiness or submission. Pure checklist equality
  and before-reservation rejection fixtures exercise that separation.
- This record precedes the #91 commit and publication. It claims no operated
  deployment, remote trace fetch, model run, lifecycle archive, full workspace
  test, Octet, release-readiness or flake qualification; the operated service
  remains at the separately deployed #90 revision.

## Final lifecycle and cleanup observations

`cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"`
passed after accepted-spec sync and completed task recording: no issues.
Native traceability coverage exited 1 with the same 71 unrelated dangling IDs
and two missing anchors already recorded for #90:
`kiln.github.landing.merge.scenario.landed` and
`kiln.github.source_status.scenario.close`. No
`kiln.review.session_traces` requirement/scenario was dangling or missing.
[`lifecycle.json`](lifecycle.json) retains a bounded projection of those native
results; it is not a replacement gate or a passing whole-repository claim.

All three owned rehearsal services exited with code 0 and their browser tabs
were closed. The temporary example seed had already returned byte-identically
to its original file snapshot. Only the permanent feature, tests, typed profile
fixtures, operator/changelog updates and lifecycle evidence remain.
