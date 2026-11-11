# Add exact base-head landing evidence

## Why

Issue #48 requires base movement to invalidate stale admission. The read-only
observer checked PR head and base ref name but not the current base commit.

## What Changes

- Bind `base_head` in landing plans.
- Read the exact base branch ref and require equality.
- Refuse moved or malformed base heads.
- Extend docs and fixture tests.

## Impact

- **Files**: `crates/kiln-adapter-github/src/landing.rs`, docs and specification.
- **Testing**: exact base head and moved base refusal.
- **Authority**: read-only evidence only; no merge or base stabilization claim.
