## ADDED Requirements

### Requirement: Source-only qualified-source publication

r[kiln.hosting.source_only_publication] A source-only publication MUST fast-forward the canonical branch to one frozen source revision without force or history rewrite, bind the exact source archive, path manifest, hosting profile, license, qualification and gate results, and record signed refs, governance, acquisition readbacks, receipt and rollback. It MUST NOT mutate mirrors, governance, announcements, deployment or release state.

#### Scenario: Qualified source fast-forwards

r[kiln.hosting.source_only_publication.scenario.accepted]
- GIVEN a frozen qualified source candidate and explicit operator authority
- WHEN publication runs
- THEN canonical `main` MUST fast-forward to that exact revision
- AND every required native and read-only route MUST reproduce the revision, path manifest and archive identity

#### Scenario: Authority, ancestry or readback differs

r[kiln.hosting.source_only_publication.scenario.rejected]
- GIVEN missing or narrower authority, a non-fast-forward revision, or a readback whose revision, paths or archive differs
- WHEN publication admission or readback runs
- THEN it MUST reject the publication or withhold the readback claim without accepting a different revision
