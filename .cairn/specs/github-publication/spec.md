# GitHub Publication Specification

## Purpose

Defines the `github-publication` capability.

## Requirements

### Requirement: Idempotent verified-PR publication

r[kiln.github.publication] Kiln MAY publish an admitted publication plan to
GitHub. A candidate branch MUST be read before creation; an absent branch MUST
be created at the exact verified candidate, while an existing branch MUST
already point at that candidate. Pull requests MUST be reconciled by exact open
head branch and base branch: zero matches creates one, one exact match is
reused, and multiple or mismatched matches MUST fail or remain unknown. The
client MUST NOT force-push, update an existing branch, merge, land, approve or
mutate issue status. Observed branch and PR identities MUST be recorded only
after the exact remote objects are observed.

#### Scenario: Missing branch and PR are created

r[kiln.github.publication.scenario.create]
- GIVEN no candidate branch or matching PR
- WHEN publication runs
- THEN the branch MUST be created at the verified candidate
- AND exactly one PR MUST be created for the exact base

#### Scenario: Existing exact branch and PR are reused

r[kiln.github.publication.scenario.existing]
- GIVEN an exact candidate branch and one matching open PR
- WHEN publication runs
- THEN no mutation MUST occur and the existing PR MUST be reused

#### Scenario: Conflicting branch or ambiguous PR

r[kiln.github.publication.scenario.refused]
- GIVEN a branch at another head or multiple matching PRs
- WHEN publication runs
- THEN it MUST refuse or retain unknown state without overwriting
