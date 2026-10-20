# Nix Eval Jobs Onix Cohort Specification

## Purpose

Defines the `nix-eval-jobs-onix-cohort` capability.

## Requirements

### Requirement: Onix evaluation uses a separate explicit profile

r[kiln.nix_eval_jobs_onix.profile] Kiln MUST expose Onix Nix job discovery only through a separately named profile and MUST NOT auto-select an evaluator from jobset contents or host capabilities.

#### Scenario: Caller selects the Onix profile

- GIVEN a request explicitly names the supported Onix profile
- WHEN request admission runs
- THEN Kiln MAY evaluate the complete Onix cohort facts.

#### Scenario: Caller omits a profile

- GIVEN an Onix-only jobset and no explicit Onix profile selection
- WHEN request admission runs
- THEN Kiln MUST reject or use the existing stock policy without inspecting the jobset to switch evaluators.

### Requirement: Onix profile binds a complete immutable cohort

r[kiln.nix_eval_jobs_onix.cohort] The Onix profile MUST bind exact `nix-eval-jobs` executable bytes, Onix Nix revision and version, platform, Wasm host ABI identity, experimental feature set, plugin bundle and BLAKE3, cohort manifest, argument policy, source, and license facts.

#### Scenario: Complete cohort matches

- GIVEN every executable, fork, ABI, feature, plugin, manifest, platform, and policy fact matches the profile
- WHEN cohort preflight runs
- THEN the adapter MAY continue to source and process admission.

#### Scenario: One cohort member drifts

- GIVEN any bound executable, fork, ABI, feature, plugin, manifest, platform, or policy fact is missing, mutable, or different
- WHEN cohort preflight runs
- THEN the adapter MUST deny before jobset evaluation.

### Requirement: Request and observation identities include Onix facts

r[kiln.nix_eval_jobs_onix.admission] Pure request admission and observation identity MUST include evaluator kind, cohort manifest, host ABI, experimental feature set, plugin identity, immutable source, selector, system, pipeline, policy, bounds, and existing discovery facts.

#### Scenario: Equal Onix requests are admitted

- GIVEN two requests have equal complete Onix and existing adapter facts
- WHEN identity construction runs
- THEN their request identities MUST be equal.

#### Scenario: Plugin identity changes

- GIVEN otherwise equal requests bind different plugin bytes or host ABI identities
- WHEN identity construction runs
- THEN their request identities MUST differ or admission MUST fail.

### Requirement: Onix execution reuses the bounded adapter shell

r[kiln.nix_eval_jobs_onix.shell] The shell MUST launch only the exact admitted executable through Bounded Exec with a clear environment, fixed home, explicit plugin paths, fixed configuration, stream bounds, deadline, cancellation, teardown, and GC-root ownership.

#### Scenario: Exact executable runs

- GIVEN the admitted executable and plugin files match preflight
- WHEN the shell launches discovery
- THEN process and stream observations MUST bind the complete Onix cohort.

#### Scenario: Ambient evaluator is available

- GIVEN another `nix`, `nix-eval-jobs`, or plugin is visible through `PATH` or host configuration
- WHEN shell resolution runs
- THEN the shell MUST ignore it and MUST NOT include it in the attempt.

### Requirement: Unsupported capability and drift do not fall back

r[kiln.nix_eval_jobs_onix.fallback] Stock-profile `builtins.wasm` use and Onix-profile feature, ABI, or plugin drift MUST produce stable profile-specific outcomes and MUST NOT retry through stock Nix, another fork, another plugin, or an ambient command.

#### Scenario: Stock profile sees builtins.wasm

- GIVEN a jobset requires `builtins.wasm` under the stock profile
- WHEN discovery admission or evaluation classifies the result
- THEN Kiln MUST report unsupported evaluator capability instead of general job failure.

#### Scenario: Onix plugin ABI is stale

- GIVEN the Onix executable and plugin host ABI do not match
- WHEN preflight runs
- THEN Kiln MUST deny before launch and MUST NOT select another evaluator.

### Requirement: Stock profile compatibility remains unchanged

r[kiln.nix_eval_jobs_onix.compatibility] The existing stock cohort, default selection, identities, fixtures, dependencies, and non-claims MUST remain unchanged and MUST build without Onix Nix or plugin artifacts.

#### Scenario: Stock-only build runs

- GIVEN the Onix profile feature and artifacts are absent
- WHEN standalone core and stock adapter validation runs
- THEN existing stock behavior MUST pass without an Onix dependency.

### Requirement: Onix fixtures are immutable and bounded

r[kiln.nix_eval_jobs_onix.fixtures] The focused rail MUST use an immutable OnixOS jobset that invokes the actual Nickel plugin through `builtins.wasm` and MUST include negative stock-profile, missing-feature, wrong-ABI, stale-plugin, malformed-stream, timeout, cleanup, fallback, and overclaim fixtures.

#### Scenario: Onix fixture discovers jobs

- GIVEN the exact Onix profile and immutable positive jobset
- WHEN focused discovery runs
- THEN admitted derivation records MUST enter the existing pure task mapping path.

#### Scenario: Fixture depends on a sibling worktree

- GIVEN the fixture or cohort resolves through an ambient sibling path or mutable revision
- WHEN fixture admission runs
- THEN the fixture MUST fail before compatibility evidence is accepted.

### Requirement: Onix adapter keeps the core and shell split

r[kiln.nix_eval_jobs_onix.functional_core] Profile admission, request identity, stream admission, task mapping, failure classification, fallback denial, and non-claim validation MUST stay pure, while executable and plugin observations, processes, pipes, clocks, cancellation, hashing, roots, and output MUST stay in shells.

#### Scenario: Pure admission runs without Onix installed

- GIVEN complete in-memory profile and request facts
- WHEN core admission runs
- THEN it MUST return the same decision without an evaluator, plugin, file, process, network, environment, clock, or credential effect.

### Requirement: Onix compatibility evidence preserves non-claims

r[kiln.nix_eval_jobs_onix.nonclaims] Passing Onix profile checks MUST NOT claim Nix or Nickel correctness, source trust, complete dependencies, cache truth, build success, output correctness, reproducibility, deployment safety, or release eligibility.

#### Scenario: Passing discovery is labeled as build proof

- GIVEN a passing Onix discovery receipt is labeled as successful build or deploy evidence
- WHEN non-claim validation runs
- THEN the evidence MUST fail.
