# GitHub landing observation

`kiln-adapter-github::landing` reads one pull request and its check runs to
produce bounded landing evidence. It is strictly read-only: no merge, approval,
branch, label or comment mutation exists in this module.

A `LandingPlan` binds the repository, PR number, base ref, exact head commit,
required check names and operation identity. An observation is admitted only
when the PR is open, unmerged, mergeable, and at the exact head/base, every
required check is unique, completed successfully on that head, and the latest
review per actor has no changes requested. Required approvals count distinct
reviewers excluding the PR author. Missing, stale, duplicate or failed checks
and insufficient approvals are refused. Check pagination beyond the bounded page
is refused rather than treated as absence of failures.

## Non-claims

The observation is not merge authority, approval authority, integration proof or
authorization to land. The separate landing authority must recheck policy,
remote state and the exact evidence before any effect. This module never merges.
