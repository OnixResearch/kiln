## Context

`crates/kiln-cli/src/main.rs` answers `--help`, `-h` and `help` with one usage text and refuses a trailing argument. Requirement `kiln.cli.help` in the `operator-cli` capability covers that. `--version` falls through to `unsupported command`. The archived help change chose not to add it.

## Decisions

### Decision: Print the binary name and the package version

**Choice:** Print `kiln ` followed by `CARGO_PKG_VERSION` and a newline. The text is assembled at compile time.

**Rationale:** This is the conventional single line, it needs no runtime input, and the workspace version is the version the changelog and release notes use.

### Decision: No source revision in the output

**Choice:** Do not print a commit identifier.

**Rationale:** A build script that reads version-control state would fail or lie in a source archive and in the Nix sandbox, where no repository exists. `SECURITY.md` asks a reporter for the tested revision separately.

### Decision: Two spellings, no bare word

**Choice:** Accept `--version` and `-V` as the only argument. `version`, `-v` and other near misses stay unsupported commands.

**Rationale:** `--version` and `-V` are the conventional pair. A bare `version` word would reserve a command name that has no other use, and lower-case `-v` conventionally means verbose.

### Decision: Same refusal as help

**Choice:** A version request followed by another argument prints the usage text on standard error with the usage exit status. Help and version share one helper for that rule.

**Rationale:** One rule for requests that take no argument. The helper removes the duplicated check.

### Decision: Only the operator CLI

**Choice:** Add the flag to `kiln` and to no other program.

**Rationale:** `kiln` is the program the quick start and the demo run. The other programs print their version nowhere today; giving all of them a flag is a separate change with its own requirement per program.

## Risks / Trade-offs

- `kiln` now has a flag the other shipped programs lack. The changelog states it.
- The version line does not identify a build. A reader with a source checkout must still name the revision.
- The usage text grows by two lines; the help tests compare the three help spellings with each other, not with a fixed text.

## Verification Strategy

Run the binary tests: both spellings print exactly `kiln` and the package version on standard output with exit status 0 and empty standard error; the usage text names `--version`; a version request with a trailing argument writes the usage text to standard error only and exits with the usage status; `-v`, `version` and `--ver` stay unsupported commands. Run the same tests against the parent source to confirm they fail there. Run the package's tests and Clippy, observe the built binary, and run traceability coverage, the planning gates and `cairn validate`.
