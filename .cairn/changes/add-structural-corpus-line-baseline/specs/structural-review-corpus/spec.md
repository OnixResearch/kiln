# Structural Review Corpus Specification

## ADDED Requirements

### Requirement: Pinned line-agreement baseline

r[kiln.review.structural.corpus] The repository MUST keep an out-of-tree harness
that reads OmniDiff's pinned fixture corpus, projects the `Full` text mappings
and Kiln's structural novel rows to per-line touched labels, and writes one CSV
row per parsed fixture. The harness MUST accept a baseline and fail when a
fixture starts falling back or either old/new mismatch count grows. The corpus
MUST be fetched by pinned revision, not vendored.

#### Scenario: Baseline regression

- GIVEN a parsed fixture with a baseline mismatch count
- WHEN a later run reports more old or new mismatched lines
- THEN the harness MUST exit non-zero and name the fixture.

#### Scenario: New fallback

- GIVEN a fixture that previously parsed structurally
- WHEN a later run returns a structural fallback
- THEN the harness MUST exit non-zero.

#### Scenario: Deterministic output

- GIVEN the same corpus and baseline
- WHEN the harness runs twice
- THEN both CSV outputs MUST be byte-identical.
