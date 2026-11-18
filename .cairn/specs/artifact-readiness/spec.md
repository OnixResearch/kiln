# Artifact Readiness Specification

## Purpose

Defines the `artifact-readiness` capability.

## Requirements

### Requirement: Real artifact readiness benchmark
r[kiln.readiness.benchmark]
Kiln MUST provide bounded real-provider readiness measurement without changing cache or execution semantics or claiming unobserved performance.

#### Scenario: Exact reproducible identity
r[kiln.readiness.benchmark.scenario.identity]
- GIVEN an explicit owned output root and typed workload
- WHEN a cohort runs
- THEN exact source/workload/provider/profile/runner/CPU/storage/cache identities and commands MUST be retained, with predeclared decision thresholds and minimum samples

#### Scenario: Equal-byte and cold/warm controls
r[kiln.readiness.benchmark.scenario.controls]
- GIVEN equal regular-content bytes with different files, symlinks and shared references
- WHEN supported local Nix/hosted paths run
- THEN actual counts and payload/archive bytes MUST be retained; cold/warm state MUST be explicit; unavailable tiers MUST remain absent rather than zero-time

#### Scenario: Observe actual phases
r[kiln.readiness.benchmark.scenario.phases]
- GIVEN real restore, realization and publication operations
- WHEN time and injected delay are observed
- THEN elapsed and CPU MUST remain separate, the delay MUST be attributed to its named actual phase, coalesced observations MUST be labeled, failures MUST remain recorded and no cache/admission/retry/fence/teardown/publication semantics may change

#### Scenario: Independent honest analysis
r[kiln.readiness.benchmark.scenario.evidence]
- GIVEN retained bounded raw successful and failed samples
- WHEN independent analysis runs
- THEN comparable cells MUST report counts, median/range/ECDF and sufficient-sample tails, never mix runner cohorts, and return no-go or insufficient evidence instead of an unsupported optimization claim
