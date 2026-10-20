## Why

Parsed claim presence admits the `att` field, but individual parsed capability entries still rely on shell-local serde extraction before capability semantics run. The closeable fact is finite capability-list shape: a non-empty list where each entry has resource and ability fields before core construction.

## What Changes

- Add a vendored verified capability-list-shape primitive over shell/serde-supplied presence facts.
- Route parsed `att` list shape through a UCAN bridge before capability construction.
- Preserve resource-string semantics, ability wildcard semantics, caveat meaning, and authorization checks as separate boundaries.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, `src/token.rs` capability parsing path, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a parsed capability-list shape bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
