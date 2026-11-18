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
with no observable merge commit remains `Unknown`. A rerun first checks whether
the PR is already merged at the exact candidate/base and, when it is, returns the
existing receipt without another merge request. A rerun after source
reconciliation returns the retained integration receipt. The method is not exposed as
an operator command and does not close issues or mutate source status.

This strategy uses GitHub's head SHA condition only. It does **not** make the
base-head check atomic with the merge, and it does not prove branch-protection
or merge-queue semantics. Those remain qualification requirements.

## Stacked merge recovery

`GithubLandingClient::recover_stack_landed` reads a published stack from the
bottom without any mutation. It returns an admitted `StackLandingReceipt` for
the merged bottom prefix, or `None` when no layer has merged. Each merged layer
must keep its verified head. Its merge commit must have exactly two parents, the
frozen base for the bottom layer or the merge below for every other layer, then
the layer's candidate, and its tree must equal the candidate's. The trunk must
equal or descend from the topmost recovered merge.

| Observation | Result |
| --- | --- |
| Merged layer whose head differs from the plan | Refused, `github_stack_layer_head_changed` |
| Merge with other parents or another tree | Refused, `github_stack_merge_mapping` |
| Merged layer above an unmerged one | `Unknown`, `github_stack_out_of_order` |
| Trunk that does not descend from the topmost merge | `Unknown`, `github_stack_trunk_unknown` |

The core's `admit_stack_integration` checks the same prefix against the plan,
and the receipt is complete only when every layer landed.

GitHub documents that a stack merges from the bottom up and that the pull
requests left above a partial merge are retargeted to the trunk and rebased.
**Rebase stack** also rewrites heads. None of these keeps a verified commit, so
none yields an integrated layer. Whether a whole-stack merge keeps the original
heads and this two-parent mapping has not been qualified on live GitHub. Until
it is, such a merge is refused or `Unknown`, never integrated.

`observe_stack` feeds the bottom-up `stack_gate`: the integrated prefix, then
each other layer's observation above. An admitted observation is `Ready`, a
domain refusal is `Refused`, and any other failure is `Unknown`. The lowest
refused or unknown layer holds every layer above it.

## Non-claims

The observation is not merge authority, approval authority, integration proof or
authorization to land. The separate landing authority must recheck policy,
remote state and the exact evidence before any effect. This module never merges.

Stack recovery proves only the stacked merge mapping of the exact planned
layers. It does not reread the merged layers' checks or approvals; those are
observed per layer before landing.
