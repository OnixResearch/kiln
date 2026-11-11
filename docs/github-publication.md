# GitHub verified-PR publication

`kiln-adapter-github::publication` implements idempotent verified-PR delivery
for an already prepared publication plan.

- The repository MUST be exactly `owner/name` using bounded GitHub-safe
  characters; malformed paths are refused before any request.
- A candidate branch is read first. If it is absent it is created at the exact
  verified candidate; if present it must already point at that candidate.
- Pull requests are reconciled by exact head branch, base branch and open state.
  Zero matches creates one; one exact match is reused; more than one or a
  different head is refused.
- The operator shell records the observed branch/PR identities in order on the
  verified attempt.

The client never force-pushes, updates an existing branch, merges, lands,
approves or mutates issue/PR status.

## Non-claims

Successful publication proves only the observed branch and PR objects returned
by GitHub. It is not integration, landing, release eligibility or verification
of the candidate's content. Remote ambiguity remains `Unknown` until reconciled.
