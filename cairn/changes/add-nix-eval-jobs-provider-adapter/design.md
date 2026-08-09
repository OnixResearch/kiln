# Design: nix-eval-jobs provider adapter

## Context

Kiln separates provider-neutral CI decisions from source, workflow, build, status, and evidence adapters. Provider observations cannot mutate canonical state without domain admission.

`nix-eval-jobs` is a standalone form of Hydra's evaluation tool. Its coordinator evaluates a Nix jobset through worker processes and emits one JSON object per discovered job.

Useful records include attribute names, derivation paths, outputs, dependencies, metadata, errors, and advisory cache information. The executable links Nix private libraries and tracks a specific Nix release cohort.

## Goal and Success Contract

The goal is optional, bounded discovery of conventional Nix jobsets as Kiln task proposals.

Completion requires these results:

- Kiln core remains independent from Nix and `nix-eval-jobs` code.
- One exact adapter profile admits the executable and evaluator cohort.
- The source and selector are immutable and explicit before process launch.
- Stream parsing is incremental and bounded.
- Successful job records become typed task proposals through a pure mapping core.
- Job errors remain visible and cannot become task success.
- Fatal or partial process outcomes remain distinct from complete evaluation.
- Upstream cache status remains advisory.
- Derivation GC roots remain owned until a checked handoff or cleanup result.
- Every external and Kiln identity remains separate.
- Positive and negative fixtures cover every declared capability.

False completion includes these results:

- importing Nix or Hydra types into core crates;
- resolving an executable through ambient `PATH`;
- evaluating a floating flake reference;
- accepting malformed lines because later lines parse;
- treating EOF after a crash as complete evaluation;
- creating tasks before pure admission;
- treating `isCached` as trusted build completion;
- deleting GC roots before downstream handoff; or
- claiming Onix Nix compatibility without exact evidence.

## Approach Registry

| Family | Mechanism | State | Reason |
|---|---|---|---|
| native core integration | Link Nix evaluator libraries into Kiln | rejected | It breaks provider neutrality and creates Nix ABI coupling. |
| source copy | Copy the upstream evaluator and protocol code | rejected | It creates license, maintenance, and private-library coupling. |
| optional process adapter | Run one pinned executable and map bounded observations | selected | It keeps compatibility code in a narrow shell. |
| Mantle-only conversion | Require all Nix jobsets to become Mantle catalogs first | deferred | It is stronger for native Mantle builds but does not cover conventional external Nix CI directly. |

## Decisions

### Decision: Implement a workflow-discovery adapter

**Choice:** The adapter supplies jobset discovery through the workflow provider boundary. It does not implement build execution, source authority, cache admission, or task completion.

Successful job observations become task proposals. A selected build provider later receives an admitted Kiln effect for each task.

**Rationale:** Evaluation discovers work. It does not prove that work was built or accepted.

### Decision: Pin the executable and evaluator cohort

**Choice:** A typed profile names an absolute executable path, expected executable BLAKE3, upstream revision, license, `nix-eval-jobs` version, Nix version, supported platform, and argument policy.

The shell observes the executable and version before launch. Any mismatch makes the adapter unavailable. No ambient executable fallback is allowed.

**Rationale:** Nix private-library ABI drift can change evaluation and output behavior.

### Decision: Admit immutable source and selector facts

**Choice:** The request binds one exact source revision or admitted local source identity, flake or expression mode, root selector, system, evaluation options, and policy identity.

Profiles reject floating references, implicit registries, unrestricted `--apply`, undeclared imports, and unsupported recursive traversal.

**Rationale:** A stable task set requires stable evaluation inputs.

### Decision: Parse the line stream incrementally

**Choice:** The shell reads complete newline-delimited records under named line, total-byte, record-count, diagnostic, metadata, and collection bounds.

A pure parser-admission core validates the supported upstream schema and converts each record into a typed observation. Unknown mandatory fields, duplicate identities, malformed JSON, truncation, invalid paths, and bound violations fail closed.

**Rationale:** A large jobset must not require one unbounded output buffer.

### Decision: Separate stream progress from terminal completeness

**Choice:** Parsed records remain provisional until the shell observes EOF, process exit, stderr status, and GC-root state under policy.

A successful process exit can complete the jobset observation. A crash, signal, timeout, parser error, or output failure produces a partial or fatal adapter outcome.

Already parsed records remain diagnostic observations. They do not become a complete task set unless explicit partial-admission policy permits exact individual tasks.

**Rationale:** Upstream emits per-job lines but no independent terminal summary record.

### Decision: Map jobs through a pure task-proposal core

**Choice:** The core receives admitted job records, profile facts, and exact Kiln project and evaluation identities. It returns bounded task proposals or stable mapping blockers.

A task identity binds the adapter schema, source, selector, system, attribute, derivation identity, outputs, and mapping policy. Provider-native attribute and derivation paths remain typed external references.

The core rejects duplicate attributes, conflicting derivations, missing required outputs, dependency cycles, unsupported aggregate records, and exceeded task bounds.

**Rationale:** Parsing external output cannot directly create canonical CI state.

### Decision: Preserve job, fatal, and unknown outcomes

**Choice:** A per-job evaluation error maps to a typed failed job observation. It does not stop independent job mapping when the process remains valid.

Fatal setup failure, protocol loss, process ambiguity, or incomplete teardown maps to provider error or unknown outcome. It does not become many invented job failures.

**Rationale:** Job errors and process failure have different recovery and retry meaning.

### Decision: Keep cache status advisory

**Choice:** Upstream cache fields can enter bounded metrics and scheduling observations. They cannot complete a task, skip provider admission, authorize substitution, or prove output presence.

Deprecated or ambiguous fields such as `isCached` remain compatibility inputs only. Kiln projects them into explicit advisory classes with source and freshness facts.

**Rationale:** Evaluator cache observations are not Kiln build outcomes or Mantle store admission.

### Decision: Retain GC roots through checked handoff

**Choice:** The shell creates a private run-scoped GC-root directory when the profile enables derivation retention. It records ownership, paths, creation results, and cleanup state.

Roots remain until each admitted task reaches a downstream provider handoff that no longer depends on the evaluator root. Cleanup failure remains explicit and cannot become clean completion.

**Rationale:** Nix garbage collection can remove derivations between evaluation and build dispatch.

### Decision: Keep Onix Nix compatibility unsupported by default

**Choice:** The initial profile covers only its exact stock Nix cohort. Onix Nix, `builtins.wasm`, alternate stores, and fork-specific behavior remain unsupported.

A future profile needs an exact binary built for that cohort plus independent positive and negative fixtures.

**Rationale:** Similar version text does not prove evaluator or builtin compatibility.

### Decision: Use a pure core and a narrow process shell

**Choice:** Pure cores own profile admission, schema admission, mapping, identity preimages, dependency checks, partial-policy decisions, and outcome classification.

Shells own executable reads, process launch, pipes, stream reads, clocks, cancellation, signals, GC-root directories, hashing, stderr capture, and provider observations.

**Rationale:** Adapter policy and mapping remain testable without Nix or processes.

## Data Flow

```text
admitted source + selector + adapter profile
  -> executable and cohort preflight
  -> bounded nix-eval-jobs process shell
  -> complete line records + process observations + GC-root facts
  -> pure schema admission and job mapping
  -> typed task proposals or blockers
  -> canonical Kiln command admission
  -> selected build-provider effects
```

## Configuration

Typed Nickel contracts define:

- executable identity, source revision, version, license, and Nix cohort;
- source and selector modes;
- allowed arguments and environment;
- line, output, record, job, metadata, diagnostic, dependency, and task bounds;
- timeout, cancellation, teardown, and partial-admission policy;
- GC-root creation, handoff, retention, and cleanup policy;
- enabled upstream fields and schema compatibility;
- cache advisory mapping;
- platform support and non-claims.

Unknown fields, zero invalid limits, contradictions, unsupported features, and identity drift fail before launch.

## Validation

Positive fixtures cover a small jobset, several independent jobs, one job error with sibling success, metadata, dependency mapping, GC-root retention, advisory cache states, and complete task admission.

Negative fixtures cover a missing binary, wrong BLAKE3, Nix version drift, floating source, malformed JSON, oversized lines, duplicate jobs, conflicting derivations, process crash, timeout, partial output, stderr flood, GC-root failure, premature cleanup, and Onix-only builtins.

A fixture executable supplies deterministic stream and process cases. A pinned upstream integration check confirms only the admitted exact cohort.

## Risks and Trade-offs

- Private Nix ABI coupling requires one exact executable cohort. Strict profile admission makes drift visible.
- A large stream can outlive client interest. Cancellation and teardown policy must remain explicit.
- GC roots consume store space. Retention and cleanup evidence prevent silent leaks.
- Partial task admission can surprise operators. It remains disabled unless profile policy enables it explicitly.
- Upstream GPL-3.0 obligations apply to distributed upstream binaries and modified source. Kiln records provenance and avoids source copying.

## Claim Boundary

Passing this change proves only checked adapter admission, bounded parsing, pure task mapping, local process observations, and fixture compatibility for the exact pinned cohort.

It does not prove Nix evaluation correctness, source truth, complete dependency semantics, cache truth, build execution, output correctness, Onix Nix compatibility, reproducibility, or release eligibility.
