# Review Noise Specification

## Purpose

Defines the `review-noise` capability.

## Requirements

### Requirement: Declared review noise rules

r[kiln.review.noise]
A code-review profile MUST accept an optional `review.noise` list of `{ id, pattern, reason }` rules. Patterns MUST be validated with the same repository-relative glob validator as ownership patterns. Rule count, pattern bytes, id bytes and reason bytes MUST be bounded by named limits, and ids MUST be unique. Profile admission, including `kiln-web-service check`, and the Nickel contract MUST reject invalid, duplicate or unbounded rules. A profile without the field MUST load and serialize unchanged.

#### Scenario: Invalid or unbounded rules are rejected

r[kiln.review.noise.scenario.invalid]
- GIVEN a profile whose noise rule has an absolute, `..`, character-class or partial-`**` pattern, a duplicate id, an over-long pattern or reason, or more rules than the named limit
- WHEN the profile is admitted by `kiln-web-service check` or exported through the Nickel contract
- THEN admission MUST fail and no service is started with that profile

### Requirement: Noise rule identity

r[kiln.review.noise.identity]
Noise rules MUST have a BLAKE3 display identity over their canonical encoding, published with the review summary, change detail and diff annotation, so a rule change is visible and attributable. The identity MUST NOT change the review store binding or the effective policy identity.

### Requirement: Display-only collapsed noise files

r[kiln.review.noise.display]
For an exact displayed comparison, every file whose new or old path matches a noise rule MUST be annotated with the first matching rule id. The console MUST render such files collapsed and pre-marked viewed for every principal, show a visible "N noise files hidden" count with the matching rule ids, and expand with one click. The pre-viewed state MUST be derived from the annotation at display time and MUST NOT be stored as a viewed marker. Expanding, selecting and commenting on a noise file MUST work exactly as for any other file. Matching MUST use declared globs only, never content inference.

#### Scenario: Lockfile, vendored and source fixture

r[kiln.review.noise.scenario.fixture]
- GIVEN noise rules for `Cargo.lock` and `vendor/**` and a revision changing `Cargo.lock`, `vendor/dep/lib.rs` and `src/lib.rs`
- WHEN the diff is requested and rendered
- THEN only `src/lib.rs` MUST be expanded, the two other files MUST be annotated with their rule ids, and the console MUST show "2 noise files hidden" with both rule ids

### Requirement: Noise never affects authority

r[kiln.review.noise.authority]
Noise rules MUST NOT satisfy, skip or weaken ownership rules, required approvals, blocking threads, CI or queue admission. A file matching both a noise rule and an ownership rule MUST still require its owner vote.

#### Scenario: Ownership over a noise path still blocks

r[kiln.review.noise.scenario.ownership]
- GIVEN an ownership rule and a noise rule over the same changed path
- WHEN submit readiness is evaluated without the owner's approval
- THEN the owner checklist item MUST be unmet and submit MUST be refused, and it MUST become met only with the owner's eligible approval
