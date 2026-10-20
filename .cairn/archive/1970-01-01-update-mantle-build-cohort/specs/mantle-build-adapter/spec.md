# Mantle Build Adapter Delta Specification

## ADDED Requirements

### Requirement: Exact cohort refresh

r[kiln.mantle.cohort_refresh] The admitted Mantle cohort MUST name one exact published source revision and one exact executable identity, and a refresh MUST bind the revision, executable identity, vendored contract bytes, fixture record, Nix cohort check, and documentation together.

#### Scenario: Refresh binds a consistent cohort

- GIVEN a published Mantle revision whose producer contract bytes match the admitted wire contract
- WHEN the cohort is refreshed through an admitted change
- THEN the profile, vendored source record, fixture record, Nix cohort check, and documentation MUST name the same revision and executable identity.

#### Scenario: Partial refresh is rejected

- GIVEN a profile, fixture record, or Nix cohort value that still names a different revision or executable identity
- WHEN profile admission or the cohort check runs
- THEN it MUST reject the mixed cohort without admitting the producer.

### Requirement: Cohorts change only through an admitted change

r[kiln.mantle.cohort_refresh.contract] A refreshed cohort MUST preserve the admitted request, observation, receipt, and native report schemas unless the same change also extends the adapter contract and its positive and negative fixtures. The executable identity MUST be observed from a clean export of the selected revision, not from a mutable producer working tree. Revision, executable identity, fixture revision, and importer pin MUST agree; any disagreement MUST fail validation or the Nix cohort check. Historical release, publication, and verification receipts MUST retain their original meaning and MUST NOT be rewritten to describe the refreshed cohort.

#### Scenario: Refreshed executable identity is observed from a clean export

r[kiln.mantle.cohort_refresh.identity]
- GIVEN a selected producer revision
- WHEN the cohort refresh records its executable identity
- THEN the identity MUST come from a clean export of that revision
- AND it MUST NOT come from a mutable producer working tree

#### Scenario: Cohort identities agree

r[kiln.mantle.cohort_refresh.drift]
- GIVEN a refreshed profile, fixture record, importer pin, or Nix cohort check
- WHEN cohort admission or the Nix cohort check runs
- THEN the revision and executable identity MUST agree across every record
- AND any disagreement MUST fail validation

#### Scenario: Historical receipts keep their original meaning

r[kiln.mantle.cohort_refresh.records]
- GIVEN a historical release, publication, or verification receipt
- WHEN the cohort is refreshed
- THEN that receipt MUST retain its original meaning
- AND it MUST NOT be rewritten to describe the refreshed cohort
