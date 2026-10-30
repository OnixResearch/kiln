## Why

GitHub issue #7. The code-review profile (`kiln.code-review-profile.v1`, `config/web-code-review.ncl`) cannot mark paths as low-signal, so every changed file renders expanded. Kiln's own changes carry large mechanical diffs (`Cargo.lock`, `vendor/`, `cairn-policy/generated/`, `.cairn/archive/**/evidence`, fixtures), and reviewer attention should go to hand-written changes. Path ownership derives from the provider comparison, so any noise feature must stay strictly separate from authority.

## What Changes

- **Profile:** an optional `review.noise` list of `{ id, pattern, reason }` path globs. Patterns use `kiln_core::paths::valid_pattern`, exactly like ownership patterns, with named limits on rule count, pattern bytes, id bytes and reason bytes. Rule ids are unique.
- **Admission:** invalid, duplicate or unbounded rules are rejected by Rust profile admission (and therefore by `kiln-web-service check`) and by the Nickel contract, with positive and negative fixtures.
- **Identity:** the rules have a BLAKE3 display identity, `noise_identity`, shown with the review summary, change detail and diff; any rule change changes it, so it is visible and attributable. The review store binding and the effective policy identity are unchanged, so existing journals, candidates and queue entries stay valid.
- **Display:** the diff response carries a `noise` annotation (identity, rules, and the first matching rule for each displayed file, by new or old path). The console renders matching files collapsed and pre-marked viewed for every principal, under a visible "N noise files hidden" count with the matching rule IDs and a one-click expand. Pre-viewed state is derived at display time from the annotation and is never stored as a marker. Expanding or commenting works exactly as for any other file.
- **Docs:** `docs/operator.md` documents the display-only boundary.

## Impact

- **Consumers:** reviewers of changes that touch generated, vendored or lock files; operators authoring review profiles.
- **Owners:** Kiln web-service maintainers (profile admission, diff annotation, console); Kiln config maintainers (Nickel contract).
- **Affected areas:** `config/web-code-review.ncl` and fixtures, `crates/kiln-web-service/src/code_review.rs` and a new `code_review/noise.rs`, `code-review.js`/`.css`, `docs/operator.md`.

## Out of Scope

- Content inference ("looks generated" heuristics); only declared globs.
- Any effect on ownership rules, required approvals, blocking threads, CI or queue admission. A file matching a noise glob and an ownership rule still needs its owner vote.
- Changes to binary or truncated file handling.
- Stored per-principal viewed markers for noise files (the viewed-marker model is owned by issue #5).
