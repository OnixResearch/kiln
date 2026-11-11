## Why

The operator tool could admit work but not create or revoke an attempt, so the
claim/fence states were reachable only from library tests.

Issue: [#45](https://github.com/OnixResearch/kiln/issues/45).

## What Changes

- Add `claim` with a positive lease (default one hour) that prints the canonical
  attempt JSON.
- Add `cancel --attempt --fence [--cause]` using the exact fence.
- Keep dispatch, verification and publication out of the tool.
- Test claim/cancel/status round-trip through the binary.

## Impact

- **Files**: `crates/kiln-adapter-github/src/bin/implementer.rs`,
  `crates/kiln-adapter-github/tests/implementer_cli.rs`,
  `docs/implementer-service.md`.
- **Testing**: claim prints a claimed attempt with frozen base; cancel revokes it;
  status shows one inactive attempt.
- **Authority**: claim/cancel only; no execution authority.
