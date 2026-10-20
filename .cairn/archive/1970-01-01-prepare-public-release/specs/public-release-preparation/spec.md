# Public Release Preparation Delta Specification

## ADDED Requirements

### Requirement: Anonymous immutable source inputs

r[kiln.public_release.sources] The documented Kiln workspace build MUST acquire every direct and transitive source input without operator credentials or sibling checkout paths. Sources MUST retain exact revisions, license notices, and verifiable provenance. Existing imported bytes MUST NOT be silently overwritten on drift.

#### Scenario: Contributor has no credentials

- GIVEN an exact Kiln source tree and an empty source cache without operator credentials
- WHEN the documented dependency acquisition runs
- THEN every required source MUST resolve through an anonymous immutable route or the recorded in-tree import.

#### Scenario: Imported source differs

- GIVEN an existing imported source whose bytes differ from its declared immutable archive
- WHEN the import verifier runs
- THEN it MUST reject the drift without replacing the source.

### Requirement: Preserve meaningful release gates

r[kiln.public_release.gates] Public-release preparation MUST retain exact cohort identity admission, reference accepted requirements from maintained source markers, and record freshly exercised checks. A changed binary digest MUST NOT be accepted solely to make a failing gate pass.

#### Scenario: Pijul rebuild differs

- GIVEN independent builds of the admitted Pijul recipe produce different executable identities
- WHEN cohort validation runs
- THEN it MUST reject the mismatch until its cause and admitted cohort are established.

#### Scenario: Stock evaluator sees Onix facts

- GIVEN the accepted stock evaluator profile and an Onix-only request
- WHEN the maintained rejection fixture runs
- THEN it MUST reject before process launch and MUST reference the accepted stock boundary rather than an unaccepted feature requirement.

### Requirement: Scoped licensing and local evidence

r[kiln.public_release.claims] Public documentation MUST distinguish package-specific licenses and owner-only exceptions, historical publication facts, current local verification, and unperformed publication effects. Local readiness evidence MUST bind the exact validated source inputs and MUST NOT grant publication authority.

#### Scenario: Current source differs from published receipt

- GIVEN a historical operator-authorized receipt and changed local source
- WHEN current readiness is documented
- THEN the historical receipt MUST retain its original meaning and MUST NOT be presented as validation or publication of the changed source.

#### Scenario: Downstream uses a copyleft integration

- GIVEN an integration includes GPL or AGPL components with no transferable permissive exception
- WHEN a contributor reads the public license notice
- THEN the notice MUST name those components and MUST NOT imply the permissive workspace default covers every combined artifact.
