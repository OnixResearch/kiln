# Tag Bound Triggers Specification

## Purpose

Defines the `tag-bound-triggers` capability.

## Requirements

### Requirement: Exact tag trigger admission

r[kiln.triggers.tag] A trigger MAY bind an exact `refs/tags/*` reference when
push delivery is enabled. The reference MUST pass the existing reference
character rules, the project MUST admit the tag for source resolution, and
pull-request mode MUST remain branch-only. Schedules MUST reject tag references.

#### Scenario: Signed tag dispatch
r[kiln.triggers.tag.scenario.dispatch]
- GIVEN an admitted tag-bound push trigger and an admitted project reference
- WHEN a signed push event names that exact tag and revision
- THEN the trigger reserves and dispatches a run bound to the tag commit.

### Requirement: Namespace-correct filters

r[kiln.triggers.tag.filter] The Nickel trigger-filter contract MUST accept `tags`. Tag-bound
pushes MUST accept `tags` globs and reject `branches`; branch-bound pushes MUST
accept `branches` and reject `tags`; pull requests MUST reject `tags`. Tag patterns MUST admit the exact bound tag. Filter
admission MUST reject invalid combinations before automation starts.

#### Scenario: Wrong namespace filter
r[kiln.triggers.tag.filter.scenario.invalid]
- GIVEN a tag-bound trigger with a branches filter
- WHEN automation is admitted
- THEN startup MUST be rejected.

### Requirement: No release authority

r[kiln.triggers.tag.boundary] A tag-bound run MUST use existing source and
workflow semantics. It MUST NOT publish, move or trust tags as release evidence
and MUST NOT add release authority.

#### Scenario: Tag run remains bounded
r[kiln.triggers.tag.boundary.scenario.bounded]
- GIVEN a successful tag-bound dispatch
- WHEN the run is inspected
- THEN it is an ordinary CI run for the exact tag commit with no release side effect.
