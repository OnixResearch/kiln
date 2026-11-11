## Phase 1: Branch

- [x] [serial] Bind the target base branch in publication preparation. r[kiln.implementation.publication_target.scenario.refused]
- [x] [serial] Reconcile absent/exact/conflicting candidate branch heads without force pushing. r[kiln.github.publication.scenario.create]

## Phase 2: Pull request

- [x] [serial] Reuse exactly one matching open PR and refuse ambiguous/mismatched matches. r[kiln.github.publication.scenario.existing]
- [x] [serial] Add the `publish` operator command and record observed branch/PR identities. r[kiln.github.publication]
- [x] [serial] Keep merge, landing and source-status mutation outside the publisher. r[kiln.github.publication.scenario.refused]
