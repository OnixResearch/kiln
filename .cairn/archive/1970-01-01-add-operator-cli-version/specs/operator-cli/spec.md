# Operator CLI Specification

## ADDED Requirements

### Requirement: Operator CLI version
r[kiln.cli.version] The `kiln` operator CLI MUST print its name and its package version on one line on standard output and exit successfully when its only argument is `--version` or `-V`, and its usage text MUST name that request. It MUST report a version request followed by another argument on standard error with a failing exit status, and MUST NOT write to standard output in that case.

#### Scenario: Version prints one line
r[kiln.cli.version.scenario.version]
- GIVEN the operator CLI
- WHEN the operator runs `kiln --version` or `kiln -V`
- THEN the CLI MUST print `kiln`, a space and its package version as the only line on standard output, MUST write nothing to standard error, and MUST exit with status 0.

#### Scenario: A trailing argument is refused
r[kiln.cli.version.scenario.refused]
- GIVEN the operator CLI
- WHEN the operator runs a version request followed by another argument
- THEN the CLI MUST write the usage text to standard error, MUST write nothing to standard output, and MUST exit with a non-zero status.
