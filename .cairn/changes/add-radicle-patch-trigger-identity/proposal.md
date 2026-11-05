## Why

Issue #18 starts with exact patch-revision CI. The deployed Defelo broker request
already carries a patch object with commits, but the adapter discarded any patch
identity and emitted `patch: None`. A patch run could therefore never be bound
to the exact patch object it came from.

## What Changes

- Add an optional patch `id` to the bounded Defelo patch request.
- Require an exact patch identity for patch events and map it into
  `TriggerRequest.patch` as a `radicle` native reference.
- Preserve the existing first-commit, repository, branch, event, and actor
  mapping.
- Add positive and negative patch-envelope tests and update the broker
  documentation.

## Impact

- **Files**: `crates/kiln-adapter-radicle/src/defelo.rs`,
  its tests, `docs/radicle-workflow-shell.md`.
- **Testing**: patch identity mapping, missing patch identity rejection, existing
  push fixture compatibility.
- **Authority**: this only preserves a native trigger fact. It does not grant
  source trust, approval, or patch execution; the patch CI profile and approval
  gate remain follow-up work.
