## Phase 1: Implementation

- [x] [serial] Add the usage text and the three help spellings, and print the same text for a missing command. r[kiln.cli.help.scenario.help]
- [x] [serial] Refuse a help request with a trailing argument as a usage error and keep the unsupported-command refusal. r[kiln.cli.help.scenario.refused]

## Phase 2: Verification

- [x] [serial] Add binary tests for every help spelling and every refusal case, run the package's tests and Clippy, observe the built binary, run traceability coverage and the lifecycle gates, and retain bounded evidence. r[kiln.cli.help]

The delta is synchronized into `.cairn/specs/operator-cli/spec.md`:
traceability coverage reports the new markers as dangling until the requirement
is accepted. The terminal rail and the packaged `nix build path:$PWD#kiln`
output were not exercised; `evidence/verification.json` lists them under
`not_exercised`.
