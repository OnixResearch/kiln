## Why

Review CI facts currently expose one success/clean/full-run boolean per run.
The review page cannot show which named checks ran, what kind of verification
they performed, or where the artifact lives. Issue #35 asks for typed check
evidence bound to the exact commit.

## What Changes

- Add `CiCheck { name, kind, outcome, artifact_digest }` to `CiFact`; omitted
  when the shell observed no checks, preserving old serialized bytes.
- Populate checks from the exact run's workflow nodes and retained reports,
  including node outcome and report/artifact BLAKE3 when available.
- Add `Policy.required_checks` with known kinds `test`, `property`,
  `differential`, `proof`, and `build`; exact-commit passing checks satisfy the
  requirement.
- Add a conditional `checks` checklist item when requirements exist.
- Show each check's name, kind, outcome and artifact digest in the Canonical CI
  review panel.
- Reject unknown, duplicate, unbounded or malformed check requirements.

## Impact

- **Files**: `crates/kiln-core/src/change_review.rs`,
  `crates/kiln-web-service/src/engine/change_review.rs`,
  `crates/kiln-web-service/src/code-review.js`, review tests.
- **Testing**: exact-commit/kind requirement cases, policy validation, full
  core/web-service/code-review suites.
- **Authority**: checks are evidence only. A passing check can satisfy an
  explicit policy requirement, but never grants gate, merge or release
  authority by itself. Agent comment outcome measurement remains open in #35.
