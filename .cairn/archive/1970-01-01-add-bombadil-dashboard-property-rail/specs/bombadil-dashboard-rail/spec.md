# Bombadil Dashboard Rail Delta Specification

## ADDED Requirements

### Requirement: Property-based terminal rail
r[kiln.observe.dashboard.property_rail] Kiln MUST run a property-based terminal rail over the dashboard's rendered surface with a pinned property-testing tool, and MUST derive every property expectation from the staged journal fixture and the checked-in dashboard profile rather than from a recorded run.

#### Scenario: Admitted journal satisfies the derived properties
r[kiln.observe.dashboard.property_rail.scenario.match]
- GIVEN an admitted journal fixture and an admitted dashboard profile
- WHEN the property rail drives the packaged dashboard under a pseudo terminal
- THEN the finished report MUST state the observed revision, the profile identity, and every declared non-claim, and the rail MUST report no violation.

#### Scenario: Journal larger than the admitted bound stays bounded
r[kiln.observe.dashboard.property_rail.scenario.bounded]
- GIVEN an admitted journal fixture with more runs than the admitted profile admits
- WHEN the property rail drives the packaged dashboard under a pseudo terminal
- THEN the finished report MUST render at most the admitted run bound, MUST agree with its own summary, and MUST report the truncation.

#### Scenario: Journal that does not admit is refused
r[kiln.observe.dashboard.property_rail.scenario.rejected]
- GIVEN a journal fixture whose event bytes do not match their recorded event identity
- WHEN the property rail drives the packaged dashboard under a pseudo terminal
- THEN the dashboard MUST exit with a failure, MUST name the admission failure, and MUST render no report.

### Requirement: Pinned property-testing cohort
r[kiln.observe.dashboard.property_rail.cohort] The property rail MUST pin one exact revision of the property-testing tool, MUST record its license in the repository license map, and MUST execute the tool as a separate program without linking it into any Kiln package.

#### Scenario: Cohort revision and terms are recorded
r[kiln.observe.dashboard.property_rail.cohort.scenario.pin]
- GIVEN the pinned tool revision and the license map entry
- WHEN the property rail is built and run
- THEN the rail MUST use exactly the pinned revision, and the license map MUST record the revision and its terms as a separately executed program.

### Requirement: Property rail non-claims
r[kiln.observe.dashboard.property_rail.nonclaims] The property rail MUST NOT claim CI success, provider correctness, journal completeness, release readiness, or that its exploration covered every journal, profile, or terminal input.

#### Scenario: Rail evidence stays scoped
r[kiln.observe.dashboard.property_rail.nonclaims.scenario.scope]
- GIVEN a passing property rail run
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the checked fixtures, properties, terminal session, and tool revision.
