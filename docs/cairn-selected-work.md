# Cairn selected-work consumer

Kiln admits Cairn's `cairn.selected-work.v1` snapshot (Cairn #14) with its
Cairn #12 obligation section, retains it per campaign member and attempt, and
uses one obligation projection for worker context, campaign inspection and the
UI. Cairn owns lifecycle and obligation meaning; Kiln owns admission, attempts,
checks and effects; Lattice executes admitted attempts.

## Intake profiles

The native campaign profile (`config/implementation-campaign-drive.ncl`) names
its intake in `native_cairn.intake`:

| intake | behavior |
| --- | --- |
| `legacy_readiness_compatibility` (absent field) | The existing graph, workflow, readiness and implementation-ready reader. No typed snapshot is recorded or claimed. |
| `typed_snapshot_v1` | Each refresh reads `cairn selected-work` from the exact observed source. A pre-attempt member's stage, blockers and enumeration then follow that admitted snapshot (`typed_readiness`): implementation is selectable only when the artifact, proposal, design and tasks phases pass, a selected unit is ready and its prerequisites are ready; an unobserved, unavailable or incomplete snapshot, a held phase, blocked units or unready prerequisites hold the member with the producer's stable blocker identifiers. Legacy artifact authoring may stand only while the snapshot holds a planning phase; in-flight stages are untouched. The snapshot is retained per member and bound to the member's current attempt. |

`native_cairn.obligation_definition` (for example `verify`) selects the
producer's admitted obligation definition.

## Admission

`kiln-adapter-cairn::selected_work` runs `cairn selected-work CHANGE --root R
--policy P --disclosure portable [--task T] [--obligation-definition D
[--obligation-observations F]] [--expected-cohort C]` through the bounded Cairn
runtime with an immutable executable identity. Admission decodes the exact wire
bytes (unknown fields refused), runs the producer's `admit_portable` against the
expected cohort (the record's own cohort on a first read), re-verifies the
predicate and node-fact identities, checks the selected policy identity, and
maps the record into `kiln_core::implementation::selected_work`. The snapshot
identity is BLAKE3 over the exact admitted bytes. Producer refusals keep their
`SelectedWorkError` names; any other failure, timeout, cancellation or unknown
outcome is `unavailable`, never an empty snapshot.

## Attempts, transitions and history

`bind_attempt` retains the exact snapshot and its obligation guidance
projection for one attempt. A later snapshot is classified against the bound
contract: changed source, intent, context, verification subject, dependencies,
selection, policy, profile or Kiln integration target is a `replan`; a changed
obligation evaluation or observation is a `refresh`. The bound contract is never
rewritten; a new attempt rebinds and the old contract moves to history. Members
show `not_observed`, `unavailable` or `admitted`; an unselected obligation
definition (`not_selected`) is distinct from an unavailable read.

## Completion

`decide_obligation_completion` requires the producer evaluation to deliver
with nothing remaining, every producer-satisfied and remaining contributing leaf
to have a matching canonical check (same predicate, requirement, accepted
revision, subject, context, policy and candidate) that passed (applicability
predicates need a terminal observation), and authoritative completion to pass for
the same subject, policy and candidate. Missing, skipped, unknown, failed or
interrupted checks, stale subject, context, policy or candidate evidence,
`selection_missing` nodes and unselected alternatives produce named gaps with the
existing owner (`verification-owner`, `cairn-lifecycle-owner`,
`campaign-policy`). `evidence_observations` and `producer_observation_file`
derive the producer's observation file from the same canonical checks.
Cairn's generic Kiln workflow importer stays non-authoritative.

## Restricted display

`restricted_display` produces `kiln.selected-work-display.v1` with coverage
`restricted`. It has no conversion into an admitted snapshot, and admission
refuses its bytes.

## Lattice workflow intent

`kiln-adapter-lattice::workflow_intent` normalizes an admitted snapshot into
Lattice's `lattice.workflow-intent.v1` input:

| Lattice field | Kiln source |
| --- | --- |
| `cohort.producer_identity` | `b3:` BLAKE3 of (`kiln.lattice-intent.producer.v1`, producer schema, `cairn`) |
| `cohort.selection_identity` | `b3:` + `cohort.selection` |
| `cohort.obligations_identity` | `b3:` + `cohort.obligations` |
| `cohort.profile_identity` | `b3:` + `profile_hash` |
| `cohort.context_identity` | `b3:` + `cohort.context` |
| `cohort.source_identity` | `b3:` + `cohort.source` |
| `cohort.policy_identity` | `b3:` + `policy_hash` |
| `cohort.dependency_identity` | `b3:` + `cohort.dependencies` |
| `coverage` | `complete` only for an exported plan with no `selection_missing` node over a complete enumeration; otherwise `incomplete` |
| leaf `obligations[].id` | node `id` of each `required`, `selected_alternative` or `conditional` evidence leaf |
| `obligation_identity`, `guidance_identity`, `remaining_condition_identity` | `b3:` + node `obligation_blake3`, `guidance_blake3`, `condition_blake3` |
| `operation_class` | `named_check` for leaves; `completion_only` for `kiln.authoritative-completion` |
| `required_observations` | the leaf's evidence references and gating applicability references: `id` = predicate, `predicate_identity` = `b3:` + `reference_blake3` |
| `action_identity`, `precondition_identity` | Kiln-owned BLAKE3 over operation, node and obligation identity; over node conditions and `cohort.dependencies` |
| `kiln.authoritative-completion` | depends on every leaf; observation bound to the producer predicate identity |
| `alternatives`, `selected_alternative` | empty; the producer's selected alternative is already reflected in the included nodes |

`admit_compiled_plan` binds Lattice's compiled plan identity, echoed intent and
facts to the attempt contract and its obligations. `review_proposal` returns
`owner_readmission_required` with added, removed and changed obligations for any
plan or intent that differs; nothing a worker proposes becomes executable.

## Fixtures and non-claims

`fixtures/cairn/selected-work/` retains producer wire records composed by the
pinned producer library from synthetic inputs, their Kiln mappings, and one
attempt contract: planning, passing, missing-check, stale-subject,
changed-policy and interrupted-observation. `fixtures/lattice/workflow-intent/`
retains the normalized intent, its compile request, the plan from one local
`lattice workflow-intent compile` run and one `compare` result
(`verify.compare-dropped.json`: dropping an obligation yields
`owner_readmission_required`). The executable was a release `lattice` built from
Lattice `661499d282c2e79ebefbd0f9b35beb5f8d7795cd`; its measured compiler identity
is the request's `compiler_artifact_identity`, so a different executable is
refused. The request's `runtime_profile_identity` is a synthetic fixture value
(`b3:` BLAKE3 of
`kiln.lattice-runtime-profile.fixture.v1:lattice@661499d282c2e79ebefbd0f9b35beb5f8d7795cd`),
not a qualified runtime profile. The same executable refuses the request with
`coverage = incomplete` (`IncompleteContext`). Set
`KILN_UPDATE_SELECTED_WORK_FIXTURES=1` or `KILN_UPDATE_LATTICE_INTENT_FIXTURES=1`
only after an intentional producer change; the request, plan and compare files
are regenerated with the Lattice CLI. These are deterministic
producer/consumer fixtures, not a native `cairn selected-work` run, a live
campaign, a live Lattice execution or provider qualification; those remain with
Kiln #49.
