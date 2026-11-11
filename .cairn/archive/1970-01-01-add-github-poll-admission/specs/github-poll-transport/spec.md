# GitHub Poll Transport Delta Specification

## ADDED Requirements

### Requirement: Bounded polling admission pass

r[kiln.github.poll.admission] The operator shell MAY run one explicit bounded
polling pass that normalizes through `poll_implementation_issues` and admits the
results into the durable implementation ledger. Repeated passes MUST be
idempotent through the work ledger; closed observations MUST NOT cancel,
complete or dispatch work. A ledger failure MUST fail the pass rather than
report partial success. The pass MUST NOT schedule itself, retry, claim,
dispatch or publish.

#### Scenario: Repeated pass creates one work version

r[kiln.github.poll.admission.scenario.repeat]
- GIVEN one open issue returned by authenticated polling
- WHEN the same bounded pass runs twice
- THEN the first pass MUST admit one version and the second MUST report a duplicate
- AND one current work version MUST remain

#### Scenario: Closed observation is not completion

r[kiln.github.poll.admission.scenario.closed]
- GIVEN a closed issue observation
- WHEN the pass runs
- THEN it MUST count the observation without cancelling or completing work
