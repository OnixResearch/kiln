# Implementation work specification delta

## ADDED Requirements

### Requirement: Intake policy preflight

r[kiln.implementation.policy_check] `kiln-implementer policy-check --policy PATH --enrollment PATH` MUST read the GitHub intake policy and enrollment with the same types and JSON reader as `poll`, MUST apply the same policy validation every intake pass applies, and MUST admit one synthetic open issue from an enrolled actor that poll's record checks accept through poll's record bounds, work construction and ledger admission in an empty in-memory ledger. It MUST refuse only when no enrolled actor passes those checks. It MUST NOT open the work database, read a credential or contact the provider. Webhook intake and polling MUST apply that one policy validation before any delivery or provider call.

#### Scenario: Admissible policy

r[kiln.implementation.policy_check.scenario.admitted]
- GIVEN a policy and enrollment under which polling admits an open issue
- WHEN the operator runs `policy-check`
- THEN it MUST exit successfully and report the admitted repository key

#### Scenario: Missing field

r[kiln.implementation.policy_check.scenario.missing_field]
- GIVEN a policy whose `base` lacks `contract_version`
- WHEN the operator runs `policy-check`
- THEN it MUST fail and name the missing field with its line and column

#### Scenario: Refused policy or work

r[kiln.implementation.policy_check.scenario.refused]
- GIVEN a policy polling refuses, derived work the ledger refuses, or an enrollment without an actor a polled issue can carry
- WHEN the preflight runs
- THEN it MUST fail with the same refusal code polling or ledger admission would return

### Requirement: Diagnosable JSON input

r[kiln.implementation.json_input_diagnostics] Implementer JSON input failures MUST name the file, MUST distinguish malformed JSON from a schema mismatch, and MUST report the line and column. A schema mismatch MUST name a missing or duplicate field and the type's expected names. No message MUST echo input: an unknown member name or a value MUST be withheld.

#### Scenario: Schema mismatch

r[kiln.implementation.json_input_diagnostics.scenario.schema]
- GIVEN well-formed JSON missing a required field
- WHEN the implementer reads it
- THEN the error MUST say the file does not match its schema and name the field, line and column

#### Scenario: Malformed JSON

r[kiln.implementation.json_input_diagnostics.scenario.malformed]
- GIVEN syntactically invalid or truncated JSON
- WHEN the implementer reads it
- THEN the error MUST say the file is not valid JSON and give the syntax error, line and column

#### Scenario: Values stay private

r[kiln.implementation.json_input_diagnostics.scenario.redacted]
- GIVEN a value of the wrong type, an unknown variant or an unknown member name
- WHEN the implementer reads it
- THEN the error MUST give the kind of mismatch and the type's expected names without the input
