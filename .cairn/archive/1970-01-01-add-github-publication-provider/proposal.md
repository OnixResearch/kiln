# Add idempotent GitHub verified-PR publication

## Why

Issues #47/#48 require exact verified-candidate delivery to GitHub with
reconciliation across retries. Preparation and read-only landing observation
existed, but no branch/PR publisher.

## What Changes

- Bind a target base branch in publication plans.
- Add a GitHub publisher that reads the branch first, creates it at the exact
  verified candidate only when absent, and reuses an exact existing branch.
- Reconcile one exact open PR by head/base, create one when absent, and refuse
  ambiguity or mismatched heads.
- Record branch/PR identities only after observing them.
- Add a `publish` operator command and fixture tests.
- Never force-push, update branches, merge or mutate source status.

## Impact

- **Files**: publication plan, GitHub adapter publisher, implementer shell/CLI,
  docs and specification.
- **Testing**: missing branch/PR creation, exact reuse, conflicting branch and
  ambiguous PR refusal.
- **Authority**: verified-PR delivery only. No landing, merge, source closure or
  release claim.
