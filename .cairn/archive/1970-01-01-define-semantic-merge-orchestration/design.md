## Context

Issue #1 asks for opt-in CI orchestration that checks an already proposed merge M against an exact base O and variants A and B. Existing pieces to reuse:

- `kiln-adapter-pijul` resolves one exact local channel state and archives one native transaction view with bounded bytes. Its profile (`profile.rs`) admits only discover, resolve-exact, acquire and observe-identity. It rejects network, credentials, collaboration, triggers, status publication and mutation.
- `kiln_core::source` defines `SourceResolution`, `PijulFacts` (Merkle state, channel, ordered change sequence, before/after state, observation identity) and `validate_resolution`. That function already refuses one change hash in place of a whole channel state.
- `kiln_core::gating` binds gate results to one candidate set and policy.
- `kiln_core::workflow` defines the versioned Lattice `WorkflowEffectPlan`, validated by `validate_workflow_plan`.
- Bounded Exec owns process limits in adapter shells (Cairn, Mantle, GitHub).
- Valence projects evidence identity and linkage.

The production checker and its typed receipt are still being designed in Octet #1 (Rust), Noble #30 and Valence #1. This change therefore delivers the contract and a pure, fixture-proven core. It does not deliver a production gate.

## Goals and Success Contract

Success means a reviewed state-transition contract and a pure core that turns one frozen quartet into exactly one CI decision. The positive fixture runs end to end through Pijul acquisition, checker request, result evidence and CI decision. Every negative fixture named in the issue fails closed, with a diagnostic that names the failed binding or the unsupported scope.

False completion includes:
- a result accepted for a different role assignment, base, candidate, dependency set, policy, checker or attempt;
- any non-verified, stale, partial or late outcome satisfying a required gate;
- unknown treated as either success or a demonstrated conflict;
- a clean text merge, equal trees, passing tests or an accepted evidence envelope satisfying the semantic gate;
- a semantic result standing in for another gate;
- any new mutation surface on the Pijul adapter or any implied merge authority.

## Decisions

### Decision: Pure module in `kiln-core`

**Choice:** add `kiln_core::semantic_merge`, a `no_std` module with an `identity` submodule, a `state` submodule and tests. The checker producer port goes in a new `kiln_provider_api::semantic_merge` module.

Reasons:
- The module needs `SourceResolution`, `PijulFacts`, `SourceProviderProfile`, `ContentId`, `Diagnostic` and `WorkflowEffectPlan`, all of which live in `kiln-core`.
- CI meaning belongs to the core (README "Owned boundary").
- A new crate would add a workspace member and lock churn without adding isolation.
- Core crates still import no provider implementation.

### Decision: Immutable four-role request

**Choice:** `SemanticMergeRequest` has four named slots: `base`, `variant_a`, `variant_b` and `candidate`. Each slot is a `RoleSource { role, resolution, acquisition_profile }`, and each role's tag must match its slot. The request also carries:
- `candidate_supply`: supplier profile, proposal reference and supplied state;
- `subject`: language, unit and required coverage units;
- `property`: id, revision and assumptions;
- `inputs`: dependency closure identity and semantic model identity;
- `checker`: producer profile, checker binary, toolchain, semantic profile and receipt profile;
- `policy`: id, mode, lineage policy, admitted checkers, source profiles, suppliers, retry policy and bound ceilings;
- `bounds`: wall time, memory, log bytes, product count and output bytes.

`admit_request` validates the request and returns an `AdmittedRequest` with private fields and the computed `RequestBinding`. Callers cannot modify an admitted request; any change requires a new admission and produces a new identity.

### Decision: Role-tagged BLAKE3 binding with named components

**Choice:** `RequestBinding` holds one BLAKE3 identity per component, plus an overall `identity` over all of them:

| Component | What it binds |
| --- | --- |
| `base`, `variant_a`, `variant_b`, `candidate` | Each is hashed with its role tag, provider profile, repository, symbolic reference, exact revision, canonical tree and native facts. For Pijul the native facts are the observation identity and the ordered change sequence. |
| `candidate_supply` | The supplier, proposal and supplied state. |
| `subject` | The semantic subject. |
| `property` | The property id and revision. |
| `assumptions` | The property assumptions. |
| `dependencies` | The dependency closure identity. |
| `model` | The semantic model identity. |
| `checker` | The checker and toolchain identities. |
| `policy` | Every policy field. |
| `bounds` | The resource bounds. |

Hashing uses framed, labelled fields with a versioned domain tag.

Derived identities:
- **Attempt:** `H(attempt tag, request identity, ordinal)`.
- **Log or product:** `H(artifact tag, request identity, attempt identity, kind, content identity)`.
- **Cache key:** the request identity.

Comparison returns the first mismatching component, so diagnostics read `semantic_binding_mismatch` at `semantic_merge.binding.<component>`. Swapping A and B changes both role-tagged components, so a result produced for the swapped assignment fails on `variant_a`.

### Decision: Explicit lineage, Pijul change containment, no inferred base

**Choice:** `LineagePolicy` has two variants:
- `DeclaredRoles`: the operator declares the roles and Kiln checks no ancestry.
- `PijulChangeContainment { require_greatest_common_base, max_candidate_extra_changes }`.

Under containment, every role must carry Pijul native facts; otherwise the request fails with `semantic_lineage_unavailable`. The containment rules are:
- O ⊆ A and O ⊆ B, by change set; otherwise `semantic_lineage_base`.
- A ∪ B ⊆ M, with at most the named number of extra candidate changes; otherwise `semantic_lineage_candidate`.
- When `require_greatest_common_base` is set, O must equal A ∩ B. If A and B share a change outside O, the base is ambiguous and the request fails with `semantic_lineage_ambiguous`.

Kiln never computes or substitutes a base. Every role must be a whole channel state, not a single change hash; `validate_resolution` already enforces this. Git ancestry lineage is unsupported scope and is refused by construction, because no such policy variant exists.

### Decision: Candidate supplied and frozen through read-only acquisition

**Choice:** M enters only as an exact `SourceResolution` acquired through an admitted source profile. The profile must:
- be the role's `source_provider`;
- be listed in policy;
- use immutable resolution;
- have capabilities within {discover, resolve-exact, acquire, observe-identity};
- include resolve-exact, acquire and observe-identity.

Any mutate, collaborate, trigger, status or credential capability is refused with `semantic_acquisition_authority`. The candidate supplier must be listed in policy, and the supplied state must equal M's exact revision. The same profile rule applies to O, A and B.

The Pijul adapter gains no operations. A new adapter test proves the following refusals:
- the admitted profile refuses every forbidden `SourceCapability` through `admit_source_capability`;
- any profile that sets a forbidden `allow_*` flag or adds a forbidden capability fails admission;
- `PijulRepositoryPort` still exposes only `discover_channels` and `resolve_and_acquire`.

### Decision: Reuse Lattice dispatch, Bounded Exec and gate interfaces

**Choice:** `checker_workflow_plan` maps one attempt to the existing `WorkflowEffectPlan`:
- one `Test` node of kind `semantic-merge-check`;
- `adapter_kind` set to the checker's semantic profile;
- `operation_identity` set to the request identity;
- `idempotency_key` set to the attempt identity;
- no single-revision source context, because the quartet travels in `operation_identity`.

The plan is checked with `validate_workflow_plan`. In the shell, `CheckBounds` maps one to one onto Bounded Exec `ExecutionLimits`: timeout, and stdout/stderr byte limits. The producer is the `SemanticCheckerPort` trait (`producer`, `check(request, cancellation)`). Only a fixture producer implements it.

Valence (#1) will validate receipt identity and linkage. Kiln records the receipt identity and the bound artifacts. No new Lattice or Mantle interface is needed.

### Decision: Modes and the outcome lattice

**Choice:**
- `CheckMode::Advisory` reports the outcome and neither blocks nor satisfies.
- `CheckMode::Required` is satisfied only when all of these hold:
  - the outcome is `Verified`;
  - the result was produced by an admitted checker for the exact binding;
  - the result covers every required unit;
  - the result is fresh against a re-observed binding.

`SemanticOutcome` is `Verified | Counterexample | Unknown | Unsupported | Exhausted(bound) | Cancelled | Stale(component) | ToolError`. Each outcome is preserved in the verdict.

The verdict's `conflict_demonstrated` flag is true only for `Counterexample`. `Unknown` is neither success nor a demonstrated conflict.

### Decision: Event-sourced attempt state machine

**Choice:** follow `state + command -> events + effects` and `state + event -> state`.

Commands:
- `Dispatch`
- `ObserveResult(observation)`
- `Retry`
- `Cancel`
- `ObserveInputs(binding)`, a fresh re-observation of all components
- `UseCache(entry)`

Results are admitted only for the current pending attempt, and only when:
- the bindings match;
- the evidence kind is a semantic checker receipt;
- every log and product binding is valid;
- the coverage is complete for `Verified`.

Partial output from the right attempt records `ToolError`. Results for cancelled, superseded or completed attempts are rejected as late (`semantic_result_late`). Results naming another attempt are rejected as replays (`semantic_attempt_replay`). Once an input change marks the evaluation stale, every later result is rejected (`semantic_result_stale`).

Retry classification is pure:
- `ToolError` may retry when the policy allows it and attempts remain.
- `Exhausted` retries only if the policy allows it.
- Every other outcome is terminal.

Only deterministic outcomes are cacheable: `Verified`, `Counterexample` and `Unsupported`. A cache entry must match the exact request identity and checker.

### Decision: Independent gates and downstream revalidation

**Choice:** `combine_gates` requires every independently required gate (build, tests, lints, review) and, separately, the required semantic gate. Neither can stand in for the other. Substitute evidence kinds (clean text merge, equal trees, passing tests, evidence envelope) are rejected as `semantic_evidence_substitution`.

Every verdict carries a `RevalidationObligation`: the request identity, the exact candidate state and tree, and the expected O, A and B context. `check_revalidation` is a pure precondition for a downstream shell. It confers no authority, and `grants_mutation_authority()` is always false.

## Risks / Trade-offs

- Containment over change sets is a Pijul-native relation, not proof that M is the intended merge. Extra candidate changes, such as conflict resolutions, are bounded by policy rather than interpreted.
- Including bounds in the cache key is conservative: a verified result under different bounds is not reused.
- The fixture producer proves the orchestration only. Checker soundness, receipt typing and applicability wait for Octet, Noble and Valence.
- The Lattice plan carries the quartet only through `operation_identity`. Its single-revision source context stays unset.

## Verification Strategy

- **Core unit tests** (`kiln-core`), positive and negative:
  - request admission, swapped roles and every changed component;
  - lineage: containment, ambiguous base, unavailable facts, extra candidate changes;
  - unauthorized candidate supply and acquisition authority;
  - modes and the full outcome lattice;
  - coverage, partial output, replay, late results, cancellation, concurrent source change;
  - cache and retry classification, gate independence, revalidation, workflow plan mapping.
- **End-to-end fixture test** (`kiln-adapter-pijul/tests/semantic_merge.rs`): four Nickel-authored Pijul observations resolve through the real adapter mapping, then pass through the request, a fixture `SemanticCheckerPort`, result admission and the CI decision. The negative fixtures from the same export cover the issue's list.
- **Adapter refusal test** for every forbidden source capability.
- **Flake check** `semantic-merge-fixtures`: `nickel export` of the fixture source must equal the committed JSON.
- **Focused gates:** `cargo fmt`, `clippy -D warnings` and tests for `kiln-core`, `kiln-provider-api` and `kiln-adapter-pijul`.
