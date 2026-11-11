## Why

The durable implementer ledger was only reachable from library tests. Operators
need an explicit command to move authenticated queued deliveries into admitted
work and to inspect retained state without writing a custom program.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add a `kiln-implementer` binary with `drain` and `status` commands.
- Require absolute database, queue, secret, policy and enrollment paths.
- Drain a bounded batch through the same re-verification and admission path as
  the library consumer.
- Report retained work, attempt and active counts.
- Keep claim, dispatch, verification and publication out of the tool.

## Impact

- **Files**: `crates/kiln-adapter-github/src/bin/implementer.rs`,
  `crates/kiln-adapter-github/tests/implementer_cli.rs`,
  `crates/kiln-adapter-github/Cargo.toml`, `docs/implementer-service.md`.
- **Testing**: end-to-end drain of a signed queued delivery, ledger inspection,
  and status output with no attempts.
- **Authority**: operator tooling over admission only; no execution authority.
