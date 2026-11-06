## Why

Canonical Radicle tag sources are now admissible, but the Radicle CI project
profile still rejects every non-branch reference. A release tag cannot drive a
release build through the existing exact-source CI path.

## What Changes

- Allow `refs/tags/` in the Radicle CI project profile.
- Carry a `Tag` trigger event for tag sources while keeping `Push` for branches.
- Expose the tag name as the trigger alias.
- Add profile and trigger tests and document the tag path.

## Impact

- **Files**: provider trigger event kind, Radicle CI project profile/tests,
  source docs and fixtures.
- **Testing**: Radicle CI tests, source tests, workspace tests, Clippy and Cairn
  validation.
