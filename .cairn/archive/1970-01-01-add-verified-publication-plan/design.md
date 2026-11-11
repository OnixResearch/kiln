## Decisions

The branch is derived from the canonical work and attempt identities so callers
cannot choose a colliding branch. Preparation requires the retained verification
receipt and candidate handoff to be re-admitted, so a changed plan or work
version cannot reuse prior evidence.

Remote branch and PR observations are recorded in order. The observed head must
equal the verified candidate; a PR cannot be recorded before its branch, and a
different head is refused. The attempt is already terminal verified, so
publication records do not resume execution or create a new claim.

## Non-claims

No branch, commit or pull request is created, pushed or verified by this module.
The repository and PR observations are local evidence, not remote truth until a
provider shell independently reads them.
