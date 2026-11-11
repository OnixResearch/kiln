# GitHub Landing Specification

## Purpose

Defines the `github-landing` capability.

## Requirements

### Requirement: Read-only landing observation

r[kiln.github.landing] Kiln MAY read one GitHub pull request and its bounded
check-run page to produce landing evidence. The observation MUST bind repository,
PR number, exact head commit, base ref and operation identity, and MUST require
the PR to be open, unmerged, mergeable, at the exact head/base, with every
required check unique, completed and successful on that head. Missing, stale,
duplicate, failed or incompletely paginated checks MUST be refused. The observer
MUST NOT merge, approve, update or otherwise mutate remote state.

#### Scenario: Exact ready observation

r[kiln.github.landing.scenario.ready]
- GIVEN an open mergeable PR at the exact head/base with all required successful checks
- WHEN the observer reads it
- THEN bounded landing evidence MUST be returned

#### Scenario: Stale or incomplete observation

r[kiln.github.landing.scenario.refused]
- GIVEN a moved head, closed/merged PR, missing/failed/duplicate check or incomplete check pagination
- WHEN observation admission runs
- THEN it MUST refuse to produce landing evidence
