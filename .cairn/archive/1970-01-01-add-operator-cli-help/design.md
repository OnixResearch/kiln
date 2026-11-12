## Context

`crates/kiln-cli/src/main.rs` matches the first argument against four command names. Anything else, including `--help`, falls through to `unsupported command` with exit status 1. A missing command prints a one-line usage to standard error with exit status 2. `kiln dashboard --help` already works because the dashboard shell parses its own options.

Accepted requirements cover `kiln dashboard` and the terminal rail over `demo` and `evidence` in `dashboard-operator-surfaces`. Nothing covers help.

## Decisions

### Decision: One usage text for help and for a missing command

**Choice:** Add one `USAGE` constant that names every accepted command with its arguments and one line of description. Help prints it on standard output with exit status 0. A missing command prints it on standard error with the existing usage exit status.

**Rationale:** Two texts drift. The one-line usage had no consumer in tests, documents or the terminal rail.

### Decision: Three spellings, exact match, no trailing arguments

**Choice:** Accept `--help`, `-h` and `help` as the only argument. A help request followed by another argument is a usage error: usage on standard error, usage exit status.

**Rationale:** These are the spellings the shipped programs with help already use; `kiln-adapter-iroh` accepts all three. `kiln help dashboard` could mean a help topic; refusing it with the usage text, which points at `kiln dashboard --help`, avoids guessing. Near misses such as `--helpx` stay unsupported commands.

### Decision: Keep existing refusals unchanged

**Choice:** Keep the `unsupported command: NAME` message and exit status 1, and exit status 2 for a missing command.

**Rationale:** The change adds help. It does not reclassify existing failures.

### Decision: New `operator-cli` capability

**Choice:** Add the requirement `kiln.cli.help` in a new `operator-cli` capability.

**Rationale:** The existing operator-CLI requirements sit under `kiln.observe.dashboard` because they concern the dashboard surface and its rail. Help for the whole command is not an observation concern.

### Decision: Test through the built binary

**Choice:** Add `crates/kiln-cli/tests/cli.rs`, which runs the built `kiln` and checks standard output, standard error and the exit status.

**Rationale:** The contract is which stream receives the text and which status the process returns. Only the process boundary shows that. The test starts no external service.

### Decision: No `--version`

**Choice:** Do not add `--version`.

**Rationale:** No shipped program has it. Adding it to one program would start a second convention without a requirement.

## Risks / Trade-offs

- The missing-command text on standard error changes from one line to the full usage. No test, document or rail consumed the old line.
- The terminal rail does not drive help. The binary tests cover it; the rail's requirement names only the demo and evidence surfaces.
- The usage text describes commands in prose. A new command must be added to the text and to the test's command list.

## Verification Strategy

Run the binary tests: each help spelling prints identical usage naming the four commands on standard output with exit status 0 and empty standard error; a missing command, two near-miss spellings and two help requests with a trailing argument write to standard error only and exit non-zero with the expected status. Run the package's tests and Clippy. Observe the built binary directly. Run traceability coverage, the planning gates and `cairn validate`.
