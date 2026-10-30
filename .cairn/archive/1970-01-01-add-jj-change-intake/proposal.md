## Why

Issue #6. Branch contribution intake only admits `refs/heads/review/*` refs (or operator-mapped branches), and one branch maps to one stable change. A Jujutsu user already has a stable identity per change, yet must create and move a `review/` bookmark for every change, and nothing ties a Kiln revision to the jj change it came from. That leaves no reliable predecessor chain for the interdiff (#5), no exact "same change, new version" source for patch-set counts (#3), and no way to submit a whole jj stack at once (issue #6 extension, after Tangled's change-id stacking).

## What Changes

- **Change-id intake (optional, operator-enabled):** an authenticated write principal submits one exact jj change ID (console, or `kiln-web-service ci review-submit --jj-change <id>`). One jj change ID maps to one Kiln change. Each newly observed commit for it becomes the next revision; an already-admitted commit appends nothing.
- **Stack intake:** an ordered list of exact change IDs (bottom to top, `--jj-change` repeated with `--stack`) creates or updates one Kiln change per change ID, with dependency edges in ancestor order. Resubmission maps by change ID, appends revisions only where the commit changed, leaves unchanged and already-merged changes untouched, rebinds or reorders dependency edges, and flags (never abandons) change IDs that disappeared from the stack.
- **Git `change-id` header:** a pushed branch whose commits carry jj's `change-id` commit header is admitted through the same change-ID mapping (`--source-ref <ref> --change-id-header`), with or without a jj binary.
- **Fail-closed admission:** jj observations go through `kiln_core::source::JujutsuFacts`; divergent changes, conflicted bookmarks, conflicted commits, a missing or divergent operation head, unreachable commits, store mismatches, revsets/prefixes and the root change are refused with stable diagnostics before any provider effect.
- **Read-only jj:** Kiln invokes only a configured jj executable with `--ignore-working-copy --at-op <observed head>`, a cleared environment and named bounds; it never rebases, squashes, pushes, snapshots or merges operations.
- **Provider observations:** the observed jj operation, the jj predecessor commits, the creating operation and the commit author are recorded per Kiln revision as provider observations, never as Kiln identities.
- **Evolog view:** change detail shows every revision with its jj predecessor, operation, author, declared authorship, a link to the interdiff from the previous revision (`review_interdiff=<from>..<to>`), and the stack membership/removal flag.
- **Configuration and docs:** the Nickel review contract gets an optional `intake.jujutsu` record; `docs/operator.md` states the boundary.

## Impact

- **Consumers:** jj users contributing to a Kiln-reviewed repository; reviewers reading revision history; #5 interdiff and #3 patch-set counts consume the mapping.
- **Owners:** Kiln web-service maintainers (intake shell, console, CLI); Kiln maintainers (`kiln_core::source` jj facts, `kiln-adapter-jj`).
- **Affected areas:** `crates/kiln-core/src/source.rs`, `crates/kiln-adapter-jj`, `crates/kiln-web-service/src/code_review/{jj_intake.rs,intake.rs,model.rs,commands.rs}`, `code_review.rs` (state field, detail JSON), `code-review.js`, `developer.rs`, `config/web-code-review.ncl` and fixtures, `docs/operator.md`.
- **Compatibility:** every new profile, request and state field is serde-default and omitted when default; existing journals, profiles and review state replay byte-identically. Branch intake is unchanged.
- **Testing:** pure planner and parser tests; adapter tests against real `jj` fixture repositories built in scratch directories; web-service command tests (rewritten change, divergent/conflicted refusal, duplicates, 4-change stack, partial edit, reorder, removal, header path, read-only proof, byte-identical replay); Nickel positive and negative fixtures; console node tests.

## Out of Scope

- Kiln becoming a jj forge or mutating jj state (no rebase/squash/push/describe, no operation merge).
- Revsets or change-ID prefixes as intake keys.
- Any write authority from a change ID: target protection, compare-and-swap submission, CI and approval rules are unchanged; stack intake grants no batch-merge authority (#11 owns the queue side).
- Automatic abandonment of changes removed from a stack.
- Stack-narrowed interdiff rendering (#5 owns it; this change publishes the mapping it consumes).
