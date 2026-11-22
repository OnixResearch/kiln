## Implementation

- [x] [serial] Add `ImplementationIssuePolicy::validate` and `repository_key`, and use them in webhook intake and polling. r[kiln.implementation.policy_check]
- [x] [serial] [depends:kiln.implementation.policy_check] Add `check_poll_policy` and the `kiln-implementer policy-check` command, with core and CLI tests. r[kiln.implementation.policy_check.scenario.admitted] r[kiln.implementation.policy_check.scenario.missing_field] r[kiln.implementation.policy_check.scenario.refused]
- [x] [serial] Report the classified serde error from the implementer's `read_json` without echoing values, with unit tests. r[kiln.implementation.json_input_diagnostics] r[kiln.implementation.json_input_diagnostics.scenario.schema] r[kiln.implementation.json_input_diagnostics.scenario.malformed] r[kiln.implementation.json_input_diagnostics.scenario.redacted]

## Qualification

- [x] [serial] Document the preflight in the operator runbook and the changelog. r[kiln.implementation.policy_check]
- [x] [serial] Run rustfmt, Clippy with warnings denied on `kiln-adapter-github`, the focused tests and `cairn validate`, and record exit statuses in `evidence/gates.txt`. r[kiln.implementation.policy_check]
