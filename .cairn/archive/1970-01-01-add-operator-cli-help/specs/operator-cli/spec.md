# Operator CLI Specification

## ADDED Requirements

### Requirement: Operator CLI help
r[kiln.cli.help] The `kiln` operator CLI MUST print one usage text that names every command it accepts on standard output and exit successfully when its only argument is `--help`, `-h` or `help`. It MUST report a missing command, an unsupported command, and a help request followed by another argument on standard error with a failing exit status, and MUST NOT write to standard output in those cases.

#### Scenario: Help prints usage
r[kiln.cli.help.scenario.help]
- GIVEN the operator CLI
- WHEN the operator runs `kiln --help`, `kiln -h` or `kiln help`
- THEN the CLI MUST print the same usage text naming the `demo`, `evidence`, `host-qualify` and `dashboard` commands on standard output, MUST write nothing to standard error, and MUST exit with status 0.

#### Scenario: Unusable input is refused
r[kiln.cli.help.scenario.refused]
- GIVEN the operator CLI
- WHEN the operator runs it with no command, with an unsupported command, or with a help request followed by another argument
- THEN the CLI MUST write the usage text or the unsupported command's name to standard error, MUST write nothing to standard output, and MUST exit with a non-zero status.
