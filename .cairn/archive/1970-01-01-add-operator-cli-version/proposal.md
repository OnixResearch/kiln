## Why

`kiln --version` prints `unsupported command: --version` and exits 1. The help change left it that way because no shipped program had a version flag, and the launch demo had to tell presenters not to run it. A first-time reader runs it right after `--help`, and a bug report needs a version to quote. The report on the launch follow-up listed the missing flag under work not done, and the operator's reply was to finish the job.

## What Changes

- `kiln --version` and `kiln -V` print `kiln` and the package version on one line on standard output and exit 0.
- A version request followed by another argument is a usage error, like a help request with a trailing argument.
- The usage text names the version request.
- Add one requirement to the `operator-cli` capability.

No other program gains a version flag. The printed version is the Cargo package version; it does not name a source revision.

## Impact

- **Current consumer:** a reader following the quick start or the launch demo, and anyone filing a report against `0.1.0`.
- **Durable capability:** the operator CLI states its version; the behavior is specified and tested through the built binary.
- **Owner:** Kiln maintainers.
- **Files**: `crates/kiln-cli/src/main.rs`, `crates/kiln-cli/tests/cli.rs`, `CHANGELOG.md`, `docs/gtm/demo.md`, `docs/gtm/launch-checklist.md` and this package.
- **Testing**: run the binary tests for both spellings, for a trailing argument and for near-miss spellings; run the package's tests and Clippy; observe the built binary; run traceability coverage and the lifecycle gates.
- **Authority**: local process output only.
