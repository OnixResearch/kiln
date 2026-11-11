## MODIFIED Requirements

### Requirement: Admitted proof-type audit

r[kiln.github.event_proof.audit]
Every remaining `Admitted*` proof type MUST be sealed by a dedicated change,
with one line per type. `AdmittedResult` MUST remain a plain record because it is
embedded in persisted semantic-merge state, not the output of a check.

#### Scenario: Audit naming

r[kiln.github.event_proof.audit.scenario.names]
- GIVEN the audit record
- WHEN a reviewer reads it
- THEN `AdmittedGithubConfig`, `AdmittedRadicleConfig`,
  `AdmittedAspenRuntimeProfile` and `AdmittedJournal` MUST be recorded as
  sealed by their dedicated changes, AND `AdmittedResult` MUST be named as a
  plain record with its reason.
