## Decisions

The merge request uses GitHub's `sha` field for the PR head. Read-back is the
authority: success requires the PR to be merged at the exact head/base and the
base ref to equal the merge commit. Any HTTP ambiguity is reconciled through the
same read-back; a missing merge commit stays `Unknown`.

Only merge commits are admitted in this first route. The implementation does not
add a CLI, issue closure, source-status mutation or merge-queue qualification.

## Non-claims

The route cannot make the base-head precondition atomic with the remote merge.
It does not prove branch protection, approvals at the server, merge-queue
behavior or release eligibility.
