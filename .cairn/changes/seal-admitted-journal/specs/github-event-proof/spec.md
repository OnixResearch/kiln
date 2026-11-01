## MODIFIED Requirements

### Requirement: Admitted proof-type audit

r[kiln.github.event_proof.audit]
The change record MUST classify each remaining `Admitted*` type as a proof (to
be sealed in a separate change) or a plain record, with one line per type, and
MUST NOT change those types here. `AdmittedJournal` is sealed by this change;
the audit now names the remaining types.

#### Scenario: Audit naming

r[kiln.github.event_proof.audit.scenario.names]
- GIVEN the audit record
- WHEN a reviewer reads it
- THEN `AdmittedGithubConfig`, `AdmittedRadicleConfig` and
  `AdmittedAspenRuntimeProfile` MUST each be named as proofs to seal separately,
  AND `AdmittedJournal` MUST be recorded as sealed here, AND `AdmittedResult`
  MUST be named as a plain record with its reason
