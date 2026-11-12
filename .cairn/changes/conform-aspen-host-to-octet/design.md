## Context

`runtime/kiln-aspen-host` is checked by `mkConsumerCheck` from the pinned Octet revision with all targets, all features and warnings denied. The crate already carries narrow, reasoned allowances for a few lints and splits larger modules into directories: `cohort`, `effect`, `runtime`, `server` and `durability`, the last with a `mod.rs`. Its older integration tests open with a `register_tool` header and reasoned allowances for `non_trait_imports` and `no_panic`.

The 14 findings at `main`, all in the library:

| Lint | Where |
| --- | --- |
| module file count, 9 against a limit of 7 | `src/` |
| file length, limit 300 | `content.rs` 699, `gossip.rs` 523, `docs_index.rs` 408 |
| underscore in module filename | `docs_index.rs` |
| `usize` in public API | `content.rs:214`, `docs_index.rs:236` |
| renamed import | `docs_index.rs:11` |
| boolean naming | `docs_index.rs:271` |
| ambient clock, sentinel fallback, fallible integer fallback | `gossip.rs:196-198` |
| ignored `Result` | `gossip.rs:376`, `gossip.rs:385` |

Cargo stops at the first target that fails. With the library clean, the check reported 71 more findings:

| Target | Findings |
| --- | --- |
| `src/main.rs` | `run` is 75 lines against a limit of 70 |
| `tests/gossip_live.rs`, 40 | 18 non-trait imports, 14 `unreachable!`, 2 `.expect()` from the `#[tokio::test]` expansion, 2 functions of 77 and 79 lines, an unbounded channel, an ignored `Result`, a compound assertion, an underscore in the filename |
| `tests/docs_sync_live.rs`, 30 | 13 non-trait imports, 12 `unreachable!`, 2 clock reads, 1 `.expect()` from the `#[tokio::test]` expansion, a function of 74 lines, an underscore in the filename |

## Decisions

### Decision: Directory modules with `mod.rs`

**Choice:** Move the three modules to `src/content/`, `src/gossip/` and `src/docs/`, each with a `mod.rs`, as `src/durability/` already does.

**Rationale:** A `foo.rs` beside `foo/` keeps one file per module in `src/`, which would leave nine. `mod.rs` removes three and leaves six. No allowance is added for file count, file length or naming.

### Decision: Keep public paths, except the renamed one

**Choice:** `content::MoltenContentPort` and every `gossip::` item keep their paths through re-exports from the module roots. `docs_index` becomes `docs::index`, the layout the lint names, with the small value types in `docs::profile`.

**Rationale:** `main.rs`, `effect.rs` and the gossip integration test use the content and gossip paths. Nothing in the repository uses `docs_index` by path.

### Decision: One clock read that can fail

**Choice:** `gossip::now_ms` stays the module's only clock read, carries one allowance with a reason, and returns `Result<u64, GossipError>` with a new `Clock` variant. `encode` and `decode` propagate it; `encode_at`, `decode_at` and `validate` still take the time as an argument.

**Rationale:** The lint's own guidance is to isolate the read in a shell-boundary helper. The previous fallbacks turned a clock before the epoch into time `0` and an overflow into `u64::MAX`; either would make expiry checks pass or fail for the wrong reason.

### Decision: `stop` reports what it used to discard

**Choice:** `GossipListener::stop` drops the shutdown sender, which wakes the task's shutdown branch exactly as a sent value does, and returns a router shutdown failure as `GossipError::Transport` when the task itself ended cleanly.

**Rationale:** The send can only fail when the task has already ended, and the task's result is read next, so nothing is lost by dropping. A failed router shutdown is a transport failure the caller should see. `Drop` keeps its best-effort send; the lint exempts it.

### Decision: `u64` at inherent methods only

**Choice:** `get_live_exact` and `get_verified_blob` take `u64`. `AspenContentPort::get_exact` keeps `usize`.

**Rationale:** The trait belongs to `kiln-adapter-aspen` at its pinned revision; the lint does not flag trait implementations.

### Decision: Extract the content-store opening from `run`

**Choice:** Move the seven lines that derive the content root and open the store into `open_content_store`.

**Rationale:** It is the block the content change added, it has one input and one output, and removing it brings `run` to 69 lines without an allowance.

### Decision: Integration tests take the existing header and fix the rest in code

**Choice:** Give both live tests the header the older integration tests carry, with the `non_trait_imports` and `no_panic` allowances and no others. Resolve every other finding in code: a `#[test]` that drives a current-thread runtime built by a local helper instead of `#[tokio::test]`, helpers that keep each function under 70 lines, a bounded channel whose overflow counter the test asserts to be zero, a bounded polling loop instead of deadline clock reads, one single-predicate assertion instead of the compound one, and filenames without underscores.

**Rationale:** The two allowances are the crate's existing convention for integration tests. `#[tokio::test]` expands to a runtime build that calls `.expect`, so keeping it would need a third allowance. `src/bin/extensionfixture.rs` is the precedent for a concatenated filename.

### Decision: No-spec classification

**Choice:** Use `no-spec-delta` with change-local acceptance and an agentic self-review.

**Rationale:** No requirement, schema or bound changes. The self-review is not independent approval.

## Risks / Trade-offs

- Moving code can drop a requirement marker. Traceability coverage is rerun.
- `now_ms` callers must handle an error. In-tree callers are the gossip integration test.
- `stop` can now return an error where it returned `Ok`. Callers already handle `Result`.
- The docs-index path changes for anyone outside this repository who used it since 2026-11-12, and two test target names change.
- The live tests are restructured, not only moved. Their assertions and their order of network operations are kept; the listener test additionally asserts that its sink dropped nothing.
- The live tests use loopback endpoints; they do not cover relays or remote peers.

## Verification Strategy

Run the host's tests before the change and after it and compare the test functions. Run the Octet deny-all check at the parent revision, where it must fail, and at the change, where it must pass. Run the aggregate `octet` check and the host package check at the change. Run Clippy for the host with warnings denied. Run traceability coverage, the planning gates and `cairn validate`.
