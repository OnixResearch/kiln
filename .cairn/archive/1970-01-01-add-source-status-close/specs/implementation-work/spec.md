# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Integrated source-status closure

r[kiln.implementation.source_status] Kiln MAY close one exact GitHub issue
source only after the publication record is integrated and the plan binds the
exact work/version, attempt/fence, integration identity, source alias and close
action. The source must be one of the work's admitted GitHub issue aliases.
Closing MUST occur only after a successful integration read-back. The observed
closed receipt MUST bind the exact close plan; replay MUST be idempotent and a
different receipt MUST conflict. Source closure MUST NOT imply release
eligibility or Cairn lifecycle completion.

#### Scenario: Close after integration

r[kiln.implementation.source_status.scenario.close]
- GIVEN an integrated publication and exact source plan
- WHEN the source close is observed
- THEN the receipt MUST bind the exact plan and record `closed`

#### Scenario: Close before integration is refused

r[kiln.implementation.source_status.scenario.refused]
- GIVEN a non-integrated publication, wrong source alias, attempt/fence or integration identity
- WHEN source-status admission runs
- THEN it MUST fail before a remote close
