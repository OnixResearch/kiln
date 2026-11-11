# Add read-only GitHub landing observation

## Why

Issue #48 requires independent read-back of remote PR, head, base and check
evidence and explicit refusal of stale admissions. The repository lacked a safe
read-only observation boundary.

## What Changes

- Add a bounded `LandingPlan` and landing observation types.
- Read one PR and one bounded check-run page through the fixed GitHub API with
  no redirects/proxy, deadlines and response bounds.
- Admit evidence only for an open, unmerged, mergeable PR at the exact head/base
  with unique successful required checks.
- Refuse stale, failed, duplicate or incompletely paginated checks.
- Add fixture tests and documentation; no mutation path.

## Impact

- **Files**: `crates/kiln-adapter-github/src/landing.rs`, library export,
  docs and specification.
- **Testing**: exact ready observation, moved head, missing check, duplicate
  check, failed check and pagination bound.
- **Authority**: read-only evidence only. No merge, approval, update or landing.
