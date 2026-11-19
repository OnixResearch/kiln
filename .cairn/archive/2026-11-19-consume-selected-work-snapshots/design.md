## Context

Cairn owns lifecycle and obligation meaning; Kiln owns admission, attempts,
checks and effects; Lattice executes admitted attempts. The producer contract is
`cairn.selected-work.v1` (Cairn `docs/selected-work.md`), consumed through the
vendored `cairn_core::verified::selected_work` and
`cairn_core::verified::workflow_obligations` APIs. Kiln does not recreate
producer semantics: the producer's `admit_portable` and
`verify_obligation_node_facts`/`verify_predicate_identity` decide producer
validity; Kiln maps the admitted record and decides its own transitions.

## Decisions

### Import

`cairn-core` at the selected-work producer revision declares exact Git
dependencies on `artifact-auth-core`/`artifact-auth-ed25519` (onix-artifact over
HTTPS) and on a historical `kiln-core` revision. The root workspace lock keeps no
Git sources: the artifact import gains `crates/artifact-auth-ed25519`, a new
`cairn-kiln-core` import retains that historical `kiln-core` revision from
Kiln's own history, and root `[patch]` entries redirect both Git URLs to the
verified imports. Kiln's canonical `onix-kiln-core` never depends on that copy.
`cairn-policy/generated` is part of the verifier import because `cairn-core`
embeds those generated policies at compile time. The gate contract import
(`vendor/cairn`) stays at its current revision.

### Admission and mapping

`kiln-adapter-cairn::selected_work` reads `cairn selected-work CHANGE --root R
--policy P [--task T] [--obligation-definition D [--obligation-observations F]]
--disclosure portable [--expected-cohort C]` through the existing bounded Cairn
runtime and an immutable executable identity. Exit 1 maps the named
`SelectedWorkError` variant to a typed Kiln refusal; any other outcome is
unknown. Admission decodes the exact wire bytes (unknown fields refused), runs
`admit_portable` against the bundled producer policy and the caller's expected
cohort (the record's own cohort on first read), independently re-verifies
exported predicate and node facts, and then maps into `kiln-core` types. The
snapshot identity is BLAKE3 over the exact admitted wire bytes.

Mapping preserves: selection (change and authored task), source revision kind
and identity plus the artifact manifest, policy/profile identities, completion
inputs, portable context entries, readiness receipt and stage decisions, stable
blockers, units with typed references (requirement vs acceptance, change vs task
scope), remaining work and dependency facts, and the exported obligation plan
(binding, predicate, selected alternatives, typed nodes and evaluation).
Completion-only work (no implementation remaining, completion/sync/archive
pending) and phase readiness are queries over those facts, not inferred prose.

The existing multi-projection reader stays available as
`IntakeProfile::LegacyReadinessCompatibility`. A native Cairn campaign profile
names its intake explicitly; an omitted value keeps existing profiles on the
legacy reader, and records that profile, rather than silently upgrading.

Under `IntakeProfile::TypedSnapshotV1` a pre-attempt member's stage comes from
its own admitted snapshot: `typed_readiness` is `ready` only when the artifact,
proposal, design and tasks phases pass, a selected unit is ready and its
prerequisites are ready; `completion_only` when no implementation remains; and
`held` (with the producer's stable blocker identifiers) for an unobserved,
unavailable or incompletely enumerated snapshot, a held phase, blocked units or
unready prerequisites. `MemberObservation::apply_intake_readiness` applies it
after the legacy reader; legacy artifact authoring may stand only while the
snapshot itself holds a planning phase, and in-flight stages are untouched. The
legacy projections stay authoritative only under the compatibility profile.

### Attempt contracts and transitions

`AttemptSnapshotContract` binds attempt and work identity to the exact admitted
snapshot, its obligation guidance projection, and an optional Kiln integration
target. Contracts are immutable. A later observation is classified against the
bound contract: changed source, intent, context, policy, profile, verification
subject, dependencies, selection or target is a `replan`; a changed obligation
evaluation or observation-only identity is a `refresh`; identical cohorts are
`unchanged`. Rebinding moves the previous contract to history with its
transition; nothing in an in-flight contract is rewritten. Campaign state keeps
one `SelectedWorkHistory` per member, so current versus historical stays
explicit, and an unavailable read is recorded as `unavailable` instead of an
empty obligation list.

### One projection for every surface

`ObligationGuidanceProjection` is a pure function of one admitted snapshot:
contributing obligation nodes, their identities, required native predicates,
remaining condition and owner, and the producer's evaluation outcome. Worker
context, campaign inspection and the UI campaign projection carry that same
retained value; tests compare their identities. Guidance prose is never parsed
or executed.

### Completion

`decide_obligation_completion` connects canonical check observations and
authoritative completion to the same obligation IDs. It requires: the evaluated
snapshot keeps the contract's verification subject, context and policy; the
producer evaluation is a successful delivery with nothing remaining; every
producer-satisfied evidence node has a matching passed check bound to the same
predicate revision, subject, context and policy; applicability references have
a terminal canonical observation; and authoritative completion passed for the
same subject and policy. Missing, skipped, unknown, failed, interrupted or stale
evidence yields explicit gaps; `selection_missing` and unselected alternatives
never contribute. Kiln also derives the producer observation file from the same
canonical checks so the loop uses one evidence set.

### Restricted display

`RestrictedSelectedWorkDisplay` (`kiln.selected-work-display.v1`) carries an
audience, coverage `restricted`, redacted fields and obligation states. It has no
conversion into an admitted snapshot; resubmitting its JSON to admission fails
at decode or schema.

### Lattice intent

`kiln-adapter-lattice::workflow_intent` normalizes an admitted snapshot into a
Kiln-owned DTO that serializes as `lattice.workflow-intent.v1`: cohort
identities from the producer cohort (`b3:` prefixed), one `named_check`
obligation per contributing leaf with its obligation, guidance and condition
identities and evidence references as required observations, and one
`completion_only` obligation for authoritative completion depending on every
leaf. Selection-missing or unexported obligations produce `incomplete` coverage,
which Lattice refuses. Plan admission binds Lattice's compiled plan identity and
echoed intent to the selected obligations and attempt; any proposal whose plan
or intent differs is refused as `owner_readmission_required`.

## Risks

- The vendored producer revision is provisional until Cairn publishes its
  commit; the importer pins and records are updated in one step.
- Fixtures are synthetic producer records composed by the vendored producer;
  they are not live Cairn, Kiln campaign or Lattice qualification (Kiln #49).

## Verification

Focused `kiln-core`, `kiln-adapter-cairn`, `kiln-adapter-lattice`,
`kiln-implementer` and `kiln-web-service` tests, then the repository gates:
formatting, clippy with `-D warnings`, workspace tests, Nickel fixtures, Cairn
validate/gates/traceability and the Octet check.
