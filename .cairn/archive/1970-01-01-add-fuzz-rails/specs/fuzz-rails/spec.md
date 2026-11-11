# Fuzz Rails Specification

## Purpose

Defines out-of-workspace fuzz targets for external feed/OPML input and
repository-content highlighting.

## ADDED Requirements

### Requirement: Feed and OPML fuzz targets

r[kiln.fuzz.feeds] The repository MUST provide fuzz targets for
`feeds::parse`, `feeds::parse_opml`, Atom/RSS/OPML render-parse round-trips and
URL admission. The targets MUST assert the parser's entry/outline bounds, URL
scheme and length, and MUST not panic on arbitrary bytes. A `fuzzing` feature
MUST shrink the absolute byte, depth, outline and text bounds without changing
normal builds.

#### Scenario: Arbitrary feed bytes

- GIVEN arbitrary bytes and a bounded entry count
- WHEN the feed target runs
- THEN it MUST return or reject without panicking and MUST not exceed the
  requested entry count.

#### Scenario: Render round-trip

- GIVEN arbitrary text fields
- WHEN Atom, RSS or OPML is rendered and reparsed
- THEN the rendered XML MUST be well-formed and XML 1.0 control characters MUST
  not appear.

### Requirement: Feed smoke evidence

r[kiln.fuzz.feeds.smoke] The feed and OPML targets MUST have a documented smoke
run with a bounded duration, libFuzzer memory/time limits and a checked-in seed
corpus. Evidence MUST name the exact targets and outcome.

#### Scenario: Feed smoke

- GIVEN the documented feed fuzz toolchain
- WHEN the feed and OPML targets run for their stated duration
- THEN the evidence MUST record the targets, duration and outcome.

### Requirement: Highlight fuzz targets

r[kiln.fuzz.highlight] The repository MUST provide fuzz targets for
`highlight`, `highlight_diff` and `structural_diff`. Each MUST exercise both a
small bounds configuration and `Bounds::DEFAULT`/`StructuralBounds::DEFAULT`.
The targets MUST assert retained line and token bounds and MUST not panic or
hang under the harness timeout.

#### Scenario: Arbitrary language and content

- GIVEN a selected supported language and arbitrary bytes
- WHEN highlighting or structural diffing runs
- THEN the result MUST respect the selected bounds and return within the
  harness deadline.

### Requirement: Highlight smoke evidence

r[kiln.fuzz.highlight.smoke] The highlight, diff and structural targets MUST
have a documented smoke run with ASan/UBSan settings, a bounded duration and a
checked-in seed corpus. Evidence MUST name the exact targets and outcome.

#### Scenario: Highlight smoke

- GIVEN the documented sanitizer toolchain
- WHEN the highlight, diff and structural targets run for their stated duration
- THEN the evidence MUST record the targets, duration and outcome.

### Requirement: Smoke evidence and non-claims

r[kiln.fuzz.nonclaims] Documentation MUST provide reproducible commands for the
feed and highlight targets, including ASan/UBSan environment variables and
libFuzzer limits. Evidence MUST record the actual targets, durations, corpus and
toolchain used. A smoke pass is not a proof of parser correctness, grammar
memory safety, service confinement or release eligibility.

#### Scenario: Recorded smoke run

- GIVEN the documented toolchain and corpus
- WHEN each target runs for its stated duration
- THEN the evidence MUST name the target, duration, input corpus and outcome.
