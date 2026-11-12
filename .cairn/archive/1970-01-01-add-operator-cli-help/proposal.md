## Why

`kiln --help` prints `unsupported command: --help` and exits 1. Nine other shipped programs, including the dashboard, the web service and the GitHub and Gitea adapters, accept `--help`, and a first-time reader reaches for it before anything else. The only way to see the command list is to run `kiln` with no argument, which reports a usage error. No accepted requirement covers the operator CLI's help or refusal surface, so the behavior was never specified.

## What Changes

- `kiln --help`, `kiln -h` and `kiln help` print one usage text that names every accepted command, on standard output, and exit 0.
- A missing command prints the same usage text on standard error and keeps its usage exit status. An unsupported command keeps its message and exit status. A help request followed by another argument is a usage error.
- Add the `operator-cli` capability with one requirement for this surface.

No command is added, removed or renamed. `kiln --version` is not added; no shipped program has it.

## Impact

- **Current consumer:** a reader following the quick start or the launch demo, and the operator preparing the `0.1.0` launch.
- **Durable capability:** the operator CLI's help and refusal behavior is specified and tested through the built binary.
- **Owner:** Kiln maintainers.
- **Files**: `crates/kiln-cli/src/main.rs`, `crates/kiln-cli/tests/cli.rs`, `docs/gtm/demo.md`, `docs/gtm/launch-checklist.md`, `CHANGELOG.md` and this package.
- **Testing**: run the new binary tests for all three help spellings and for each refusal case; run the package's tests and Clippy; observe the built binary directly; run traceability coverage and the lifecycle gates.
- **Authority**: local process output only. Help reads no file, contacts no provider and dispatches no effect.
