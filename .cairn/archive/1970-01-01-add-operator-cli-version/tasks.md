## Phase 1: Implementation

- [x] [serial] Print the binary name and package version for `--version` and `-V`, and name the request in the usage text. r[kiln.cli.version.scenario.version]
- [x] [serial] Refuse a version request with a trailing argument through the rule help already uses. r[kiln.cli.version.scenario.refused]

## Phase 2: Verification

- [x] [serial] Add binary tests for both spellings, the trailing-argument refusal and near-miss spellings; confirm they fail on the parent source; run the package's tests and Clippy, observe the built binary, run traceability coverage and the lifecycle gates, and retain bounded evidence. r[kiln.cli.version]

The delta is synchronized into `.cairn/specs/operator-cli/spec.md`:
traceability coverage reports the new markers as dangling until the requirement
is accepted. The terminal rail does not drive the version request.
