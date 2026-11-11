## Why

Trigger filters support tag globs in the core, but hosted automation only
admitted `refs/heads/*` references and rejected tag filters. Operators cannot run
a release or artifact pipeline from an exact signed tag push.

Issue: [#13](https://github.com/OnixResearch/kiln/issues/13).

## What Changes

- Admit exact `refs/tags/*` trigger references in the web-automation profile.
- Validate filters according to the bound namespace: tag-bound pushes accept
  `tags` and reject `branches`; branch-bound pushes accept `branches` and reject
  `tags`; pull requests remain branch-bound and reject tags.
- Permit tags in project admitted references so source resolution can bind the
  exact tag commit.
- Keep schedules branch-bound.
- Add an end-to-end signed tag-push dispatch test and operator docs.

The run is bound to the exact tag commit and the existing push-profile source
semantics; no release authority is added.
