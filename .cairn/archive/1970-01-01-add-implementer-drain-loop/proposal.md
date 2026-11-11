## Why

Operators needed to rerun drain manually. A bounded loop makes admission
recovery operable without introducing a claim or dispatch scheduler ahead of the
Lattice implementation route.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `kiln-implementer run` with a required positive interval, optional positive
  iteration count, and SIGTERM/SIGINT shutdown.
- Reuse the same drain inputs and admission path as the one-shot command.
- Report iterations and drained counts; stop on the first drain error.
- Keep claims and dispatch out of the loop.

## Impact

- **Files**: `crates/kiln-adapter-github/src/bin/implementer.rs`,
  `crates/kiln-adapter-github/tests/implementer_cli.rs`,
  `docs/implementer-service.md`.
- **Testing**: bounded two-iteration run over an empty queue and CLI drain/status
  regression.
- **Authority**: admission only.
