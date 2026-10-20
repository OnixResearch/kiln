# Source Collaboration Specification Delta

## ADDED Requirements

### Requirement: Pijul source-to-CI E2E

r[kiln.source.pijul_e2e] The maintained Pijul native rail MUST resolve one exact local channel state, acquire its bounded snapshot, materialize only admitted entries, execute a reviewed command, admit the resulting workflow observation, and record the terminal Kiln outcome.

#### Scenario: Exact snapshot passes

r[kiln.source.pijul_e2e.scenario.pass]
- GIVEN a real Pijul repository with a recorded tracked file
- WHEN the adapter resolves and acquires the exact channel state
- THEN the snapshot MUST contain the exact file bytes and no `.pijul` metadata
- AND the materialized checkout MUST preserve those bytes
- AND a reviewed command MUST observe the exact content and exit zero
- AND the admitted workflow observation and terminal CI outcome MUST be `Success`

#### Scenario: Command failure stays a domain failure

r[kiln.source.pijul_e2e.scenario.fail]
- GIVEN the same exact Pijul snapshot
- WHEN the reviewed command verifies the content and exits nonzero
- THEN the workflow node MUST be terminal and unsuccessful
- AND `admit_workflow_observation` MUST return `DomainFailure`
- AND the recorded CI run MUST remain `DomainFailure`, not success or unknown

#### Scenario: Source mapping agrees

r[kiln.source.pijul_e2e.scenario.mapping]
- GIVEN the direct adapter observation for the selected channel
- WHEN the source mapping produces `SourceResolution`
- THEN the exact revision, repository, and selected provider profile MUST match
- AND the workflow plan MUST use that exact revision as its source context

#### Scenario: Unsafe materialization is rejected

r[kiln.source.pijul_e2e.scenario.unsafe]
- GIVEN an admitted snapshot entry with an absolute, parent, current-directory, `.pijul`, or unsupported path or kind
- WHEN materialization runs
- THEN it MUST fail before writing outside the temporary checkout
