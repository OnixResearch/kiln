# Interface Ownership Specification

## Purpose

Defines interface-scoped ownership requirements over the current affected-project
facts. It adds dependent CI and owner acknowledgement without replacing the
rule's own approval.

## Requirements

### Requirement: Bounded interface policy

r[kiln.review.interface.policy] An ownership rule MAY declare `project` and
`interface`. Interface patterns MUST be valid, unique and listed verbatim in the
rule's `paths`; an interface MUST have a project. Project identities MUST be
non-empty, trimmed and bounded. Invalid combinations MUST reject the policy.
Empty optional fields MUST serialize as absent.

#### Scenario: Interface outside paths
r[kiln.review.interface.policy.scenario.outside]
- GIVEN a rule whose interface pattern is not one of its paths
- WHEN the policy is admitted
- THEN it MUST be rejected.

### Requirement: Interface dependency requirements

r[kiln.review.interface.ci] When an interface pattern matches a current or old
changed path, review MUST require a full clean successful run for every other
affected project in the current required set at the exact commit and current
scope identity. Missing, stale, wrong-scope, non-clean or partial runs MUST leave
the requirement unmet. The requirement MUST apply even when `require_ci` is
false.

r[kiln.review.interface.ack] For each such affected project, each applicable
ownership rule linked by `project` MUST require one eligible non-author owner
approval on the exact revision. These acknowledgements are additional to the
interface rule's own owner approval and follow existing advisory/binding agent
vote policy. Automatic reviewers MUST include the dependent owners.

#### Scenario: Exact interface dependency
r[kiln.review.interface.ack.scenario.exact]
- GIVEN an interface change affecting project `web` and a rule linked to `web`
- WHEN the checklist is evaluated
- THEN `web` CI and its eligible owner approval are required at the exact revision.

#### Scenario: Non-interface path change
r[kiln.review.interface.ci.scenario.non_interface]
- GIVEN a changed path outside every interface pattern
- WHEN the checklist is evaluated
- THEN no interface dependency items are added.

### Requirement: Conservative facts boundary

r[kiln.review.interface.boundary] The first cut MUST use the shell-provided
affected-project requirements and MUST treat every other affected project as a
dependent. It MUST NOT infer a dependency edge or a class from authorship or
content. Missing affected facts MUST NOT fabricate dependents.

#### Scenario: Missing affected facts
r[kiln.review.interface.boundary.scenario.missing]
- GIVEN no affected-project requirement facts
- WHEN an interface path changes
- THEN no interface dependency item is inferred.
