# GitHub Landing Delta Specification

## ADDED Requirements

### Requirement: Reconcile merge ancestry after base advancement

r[kiln.github.landing.merge_ancestry] Landing read-back MUST accept a merge
result when the current base head equals the merge commit or GitHub's compare
read-back reports the base head ahead of or identical to the merge commit with
that merge commit as the merge base. A missing merge commit, divergent base or
unrelated merge base MUST remain unknown/refused. This ancestry check MUST NOT be
described as atomic merge-queue or base-protection qualification.

#### Scenario: Base advanced after landing

r[kiln.github.landing.merge_ancestry.scenario.advanced]
- GIVEN a merged candidate and a later base head
- WHEN compare proves the merge commit is the merge base and the base is ahead
- THEN the existing landing receipt MUST be recovered without another merge
