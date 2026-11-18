# GitHub Publication Specification

## Purpose

Defines the `github-publication` capability.

## Requirements

### Requirement: Idempotent verified-PR publication

r[kiln.github.publication] Kiln MAY publish an admitted publication plan to
GitHub. A candidate branch MUST be read before creation; an absent branch MUST
be created at the exact verified candidate, while an existing branch MUST
already point at that candidate. The candidate's objects MUST be pushed through
native Git from the exact attempt and fence export, and the branch MUST be
confirmed or read back at the candidate, before any pull-request request. Pull
requests MUST be reconciled by exact head branch and base branch across open and
closed states: zero matches creates one, one exact open or merged match is
reused, and multiple, closed-unmerged or mismatched matches MUST fail or remain
unknown. The client MUST NOT force-push, update an existing branch, merge, land,
approve or mutate issue status. Observed branch and PR identities MUST be
recorded only after the exact remote objects are observed.

#### Scenario: Candidate objects are uploaded before pull-request transport

r[kiln.github.publication.scenario.upload]
- GIVEN a captured verified candidate and an absent branch
- WHEN the branch is published
- THEN the exact candidate commit and tree MUST reach the remote branch before any pull-request request

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
