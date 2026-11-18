# GitHub verified-PR publication

`kiln-adapter-github::publication` implements idempotent verified-PR delivery
for an already prepared publication plan.

- The repository MUST be exactly `owner/name` using bounded GitHub-safe
  characters; malformed paths are refused before any request.
- The client revalidates the plan's repository, branch names, candidate commit
  shape and PR text before any request.
- Branch/PR creation races are reconciled at most once; a second race remains
  `Unknown`. Incomplete PR pagination is refused rather than treated as absence.
- Matching PRs are selected by the exact open head branch, base branch and
  candidate head; ambiguous matches are refused.
- A candidate branch is read first. If it is absent it is created at the exact
  verified candidate; if present it must already point at that candidate.
- Pull requests are reconciled across open and closed states by exact head/base.
  Zero matches creates one; one open or merged match is reused; more than one
  match or a closed-unmerged match is refused rather than creating a duplicate.
- The operator shell records the observed branch/PR identities in order on the
  verified attempt.

The client never force-pushes, updates an existing branch, merges, lands,
approves or mutates issue/PR status. A rerun after integration or source
reconciliation returns the retained PR instead of creating a second one.

## Ordered stacks

Stack publication is off by default. `kiln_core::implementation::resolve_route`
keeps the single verified pull request unless `GithubStack` is requested and
the repository has stacked pull requests. An `OriginStack` request is refused
before any effect until an Origin collaboration adapter is admitted.

A stack splits the attempt's verified candidate along its ancestry. The top
layer is that candidate with the attempt's own verification. Every lower layer
is an ancestor with its own `LayerVerification`, admitted under the same
canonical CI rules. `admit_stack_plan` binds two to sixteen layers, each
layer's parent (the frozen base, or the candidate below) and a deterministic
`kiln/stack-<hash>-<position>` branch whose base is the branch below.

GitHub's model, per its stacked pull request documentation, is that the bottom
pull request targets the trunk, each one above targets the branch below, and
`POST /repos/{owner}/{repo}/stacks` links them from bottom to top.
`GithubPublicationClient::publish_stack` runs in this order:

1. `GET /stacks?per_page=1`. GitHub answers `404` when stacked pull requests
   are not enabled, and publication is refused before any effect.
2. Every layer's captured commit must have its planned tree and descend from
   its planned parent.
3. Every layer's pull request is looked up by its head branch alone, and the
   branch of a layer without one is read. A conflicting branch or pull request,
   including one GitHub retargeted after a lower merge, is refused before the
   first push.
4. From the bottom up, a missing branch is pushed at the exact candidate and a
   missing pull request is created against the branch below.
5. `GET /stacks?pull_request=<bottom>` reuses a stack whose trunk, members,
   order, head branches and head SHAs equal the plan, and refuses any other.
   With no stack, one is created. A conflict, a lost response or an unreadable
   body is reconciled once and otherwise stays `Unknown`.

A re-run reuses the same branches, pull requests and stack and issues only
reads. Kiln never adds to, removes from, retargets, rebases or dissolves a
stack, and it never merges.

## Non-claims

Successful publication proves only the observed branch and PR objects returned
by GitHub. It is not integration, landing, release eligibility or verification
of the candidate's content. Remote ambiguity remains `Unknown` until reconciled.

The stack endpoints have not been exercised on live GitHub. Requests carry the
client's pinned `X-GitHub-Api-Version: 2022-11-28`; GitHub's stack reference
shows `2026-03-10` in its examples. A repository that does not serve the stack
endpoints is refused as unsupported. The route is not yet selectable from the
operator CLI, and stack records are not yet persisted on the attempt.
