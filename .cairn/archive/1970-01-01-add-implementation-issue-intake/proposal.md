## Why

The implementation-work contract had no authenticated source intake. GitHub
already defines signed issue deliveries; Kiln should admit them as bounded work
observations without interpreting issue text as execution permission.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add authenticated GitHub `issues` delivery admission in
  `kiln-adapter-github`.
- Reuse the existing HMAC-SHA256 authentication, delivery/header validation and
  repository policy.
- Bound the body, title, issue body and actor text.
- Require the sending actor to be enrolled for implementation.
- Parse `updated_at` as a strict UTC timestamp for the work sequence.
- Produce one bounded `ImplementationWork` from operator policy plus the exact
  event bytes; a closed issue produces an explicit closed observation.
- Ignore unsupported actions and unrelated event names without creating work.

## Impact

- **Files**: `crates/kiln-adapter-github/src/implementation.rs`,
  `crates/kiln-adapter-github/src/{lib,events}.rs`,
  `docs/implementation-work.md`.
- **Testing**: valid admission, forged signature, wrong repository, unenrolled
  actor, edit versioning, closed observation, unsupported action and unrelated
  event.
- **Authority**: intake only. Acceptance and permissions are operator policy.
  Closing and reopened events never prove implementation completion, and no
  claim, execution, verification or publication authority is added.
