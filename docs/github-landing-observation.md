# GitHub landing observation

`kiln-adapter-github::landing` reads one pull request and its check runs to
produce bounded landing evidence. It is strictly read-only: no merge, approval,
branch, label or comment mutation exists in this module.

A `LandingPlan` binds the repository, PR number, base ref, exact head commit,
required check names and operation identity. An observation is admitted only
when the PR is open, unmerged, mergeable, at the exact head/base, and the
observed base branch head equals the expected base commit, every
required check is unique, completed successfully on that head, and the latest
review per actor has no changes requested. Required approvals count distinct
reviewers excluding the PR author. Missing, stale, duplicate or failed checks
and insufficient approvals are refused. Check pagination beyond the bounded page
is refused rather than treated as absence of failures.

## Guarded merge

`GithubLandingClient::land` first admits the full read-only observation, then
POSTs a merge-commit request bound to the exact head SHA. It reads the PR and
target base ref back after the request and returns a receipt only when the PR is
merged at the exact head/base and the base head equals the merge commit. A
timeout or ambiguous response is reconciled by the same read-back; a lost effect
with no observable merge commit remains `Unknown`. The method is not exposed as
an operator command and does not close issues or mutate source status.

This strategy uses GitHub's head SHA condition only. It does **not** make the
base-head check atomic with the merge, and it does not prove branch-protection
or merge-queue semantics. Those remain qualification requirements.

## Non-claims

The observation is not merge authority, approval authority, integration proof or
authorization to land. The separate landing authority must recheck policy,
remote state and the exact evidence before any effect. This module never merges.
