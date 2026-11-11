## Context

GitHub event admission already accepts `refs/tags/*`. The project profile admits
arbitrary source references. Core trigger filters already evaluate tag globs;
the hosted trigger layer previously rejected them and validated only pull-request
filter shapes.

## Decisions

- `valid_reference` admits heads and tags for triggers; schedules are separately
  constrained to heads.
- Filter admission chooses the intended event shape: pull-request filters when
  pull requests are enabled, otherwise push filters.
- A tag reference cannot enable pull requests. A tag-bound push rejects branch
  filters, and a branch-bound push rejects tag filters. Tag filters must match the
  bound tag.
- The project's admitted references remain the source authority; a tag not listed
  there cannot be resolved by a run.
- The test creates a real local tag and signs a tag push event through the trigger
  admission, source resolution and dispatch path.

## Risks

- Annotated tags are resolved to their commit for source snapshots.
- Tag history is not used as a CI base; existing changed-file fallback semantics
  apply when no base is available.
- No tag or release publication authority is added.
