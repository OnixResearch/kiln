# Trigger Filters Specification

## Purpose

Defines the `trigger-filters` capability.

## Requirements

### Requirement: Bounded trigger filters

r[kiln.triggers.filter]
Hosted pipeline triggers and web automation triggers MUST accept optional `branches`, `tags`, `types` and `paths` lists. Each list MUST be bounded by named pattern-count and pattern-byte limits. Patterns MUST use the shared repository glob syntax. Admission MUST reject unknown keys, filters that do not apply to the trigger's event kind, and negated, character-class or alternation patterns before any execution.

#### Scenario: Invalid filter

r[kiln.triggers.filter.scenario.invalid]
- GIVEN a push trigger with `types`, a manual trigger with `paths`, a pattern starting with `!`, or an unknown filter key
- WHEN the pipeline is compiled or the automation policy is admitted
- THEN admission MUST fail and nothing MUST execute

### Requirement: Deterministic filter evaluation

r[kiln.triggers.evaluate]
Filter evaluation MUST be a pure deterministic function of the filter, the event and the changed-file facts. Branch globs MUST match the pushed branch or the pull request's base branch. Tag globs MUST match pushed tags. Types MUST match the pull request action, with empty types meaning `opened`, `reopened` and `synchronize`. Path globs MUST match when any changed path or rename old path matches. A trigger without filters MUST keep today's behaviour.

#### Scenario: Match and non-match

r[kiln.triggers.evaluate.scenario.match]
- GIVEN filters on branches, tags, types and paths
- WHEN events that satisfy and that violate each filter are evaluated
- THEN satisfying events MUST run with the matching pattern recorded, and violating events MUST be skipped naming exactly the first excluding filter

#### Scenario: Unknown changed files run

r[kiln.triggers.evaluate.scenario.unknown]
- GIVEN a trigger with a `paths` filter
- WHEN the changed files cannot be computed exactly (no base, truncated diff or read error)
- THEN the trigger MUST match with a `paths_unknown` reason naming the fallback

### Requirement: Recorded trigger decisions

r[kiln.triggers.record]
Every run started by a filtered trigger MUST record which trigger matched and the reasons. Every event excluded by a filter MUST be durably recorded as a non-run naming the excluding filter, without queuing work, reserving finance or starting a process.

### Requirement: Filtered checks never satisfy required CI

r[kiln.triggers.required]
A required check whose pipeline was skipped by a filter MUST leave the submit checklist unmet. The review policy MAY declare `optional_ci` rules naming a project and path globs. A required check without a run MUST be satisfied only when the change's changed files are complete and non-empty, and every changed path is covered by such a rule. A failed run MUST still block.

#### Scenario: Skipped required check

r[kiln.triggers.required.scenario.unmet]
- GIVEN a required project whose trigger excluded the change by path filter
- WHEN the submit checklist is computed
- THEN the CI item MUST be unmet

#### Scenario: Declared optional paths

r[kiln.triggers.required.scenario.optional]
- GIVEN an `optional_ci` rule for that project covering `docs/**`
- WHEN every exact changed file is under `docs/`, or when one file is outside it, or when the changed files are unknown
- THEN only the first case MUST satisfy the CI item

### Requirement: Actions filter migration

r[kiln.triggers.migrate]
`migrate` MUST map Actions `branches`, `tags`, `paths` and `types` onto native trigger filters. It MUST map `closed` explicitly to `closed` plus `merged`. It MUST reject `branches-ignore`, `tags-ignore`, `paths-ignore`, negation and unsupported pattern syntax with an explicit error.

#### Scenario: Round trip

r[kiln.triggers.migrate.scenario.round_trip]
- GIVEN an Actions workflow using push `branches` and `paths`, and pull_request `branches` and `types`
- WHEN it is migrated and the result is serialized and compiled again
- THEN the native triggers MUST carry the same filters and the compiled identity MUST be stable

### Requirement: Existing configuration replays unchanged

r[kiln.triggers.replay]
Existing hosted pipelines, compiled plan identities, automation profiles, policy fingerprints, source records, review policies and journals MUST serialize byte-identically when no filter is used.
