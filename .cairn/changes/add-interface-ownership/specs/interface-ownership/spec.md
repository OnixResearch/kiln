# Interface Ownership Specification

## Purpose

Defines interface-scoped ownership requirements over the current affected-project
facts. It adds dependent CI and owner acknowledgement without replacing the
rule's own approval.

## ADDED Requirements

### Requirement: Bounded interface policy

r[kiln.review.interface.policy] An ownership rule MAY declare `project` and
`interface`. Interface patterns MUST be valid and unique. Each MUST either appear verbatim
in the rule's `paths` or be a literal path matched by one of those patterns; an
interface MUST have a project. Project identities MUST be
non-empty, trimmed and bounded. A review service MUST reject interface/project, dependent and
required-evidence project links that do not name one of its configured projects. Invalid combinations MUST reject the policy.
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
the requirement unmet. The separate interface CI item MUST be added when
`require_ci` is false, when global CI does not already cover affected projects.

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
affected-project requirements. Policy MAY declare explicit `ownership.dependents`
from a source project to dependent projects; a missing source falls back to every
other affected project, while an empty set declares none. It MUST NOT infer a
dependency edge or a class from authorship or content. Missing affected facts
MUST NOT fabricate dependents.

#### Scenario: Missing affected facts
r[kiln.review.interface.boundary.scenario.missing]
- GIVEN no affected-project requirement facts
- WHEN an interface path changes
- THEN no interface dependency item is inferred.
