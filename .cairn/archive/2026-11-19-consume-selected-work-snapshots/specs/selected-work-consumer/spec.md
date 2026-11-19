## ADDED Requirements

### Requirement: Typed selected-work admission

r[kiln.selected_work.admission] Kiln MUST admit a Cairn `cairn.selected-work.v1` snapshot only through `kiln-adapter-cairn`, using the producer's portable admission against an expected cohort and independent obligation predicate and node-fact verification, and MUST map it into Kiln-owned types that preserve exact source, intent, profile, policy, context, verification-subject, evidence, selection and dependency identities. Unsupported, malformed, incomplete, stale, mixed-cohort and crossed-subject records MUST fail with a typed refusal before any effect.

#### Scenario: Valid native snapshot admits

r[kiln.selected_work.admission.scenario.valid]
- GIVEN a portable snapshot composed by the pinned producer and its own cohort
- WHEN Kiln admits it
- THEN every cohort identity and the exact wire identity MUST be retained in Kiln types

#### Scenario: Invalid snapshots refuse

r[kiln.selected_work.admission.scenario.refused]
- GIVEN an unsupported schema, malformed field, incomplete enumeration, stale cohort, mixed cohort or crossed subject
- WHEN Kiln admits it
- THEN admission MUST fail with the named refusal and no Kiln snapshot

### Requirement: Typed references and work facts

r[kiln.selected_work.references] Kiln MUST keep requirement and acceptance-criterion references as distinct kinds with their change or task scope, and MUST preserve phase readiness, completion-only work, stable blockers and dependency facts.

#### Scenario: Reference kinds stay distinct

r[kiln.selected_work.references.scenario.distinct]
- GIVEN a requirement and an acceptance reference with the same identifier in different scopes
- WHEN the snapshot maps into Kiln
- THEN the references MUST remain two distinct typed values

### Requirement: Immutable per-attempt contracts

r[kiln.selected_work.attempts] Kiln MUST retain the exact admitted snapshot and its obligation guidance projection for each attempt. A later observation MUST produce an explicit unchanged, refresh or replan transition; changed source, intent, context, policy, verifier subject or target MUST require replan; the in-flight contract and historical evidence MUST NOT be rewritten. Current and historical observations, and unavailable and empty observations, MUST stay distinguishable.

#### Scenario: Changed inputs replan without erasing history

r[kiln.selected_work.attempts.scenario.replan]
- GIVEN an attempt bound to one snapshot
- WHEN a later snapshot changes its source, intent, context, policy, subject or the target
- THEN Kiln MUST record a replan, keep the bound contract unchanged and retain it as history after rebinding

#### Scenario: Unavailable is not empty

r[kiln.selected_work.attempts.scenario.unavailable]
- GIVEN a selected-work read that fails or is unknown
- WHEN Kiln records the member observation
- THEN it MUST be recorded as unavailable, never as an empty obligation set

### Requirement: Obligation completion

r[kiln.selected_work.completion] Kiln MUST connect canonical check observations and authoritative completion to the same exported obligation identifiers. Missing, skipped, unknown, failed or interrupted checks, stale candidate, subject or policy evidence, selection-missing nodes and partial alternatives MUST NOT become completion. Worker context, campaign inspection and the UI projection MUST carry the same obligation projection for one admitted snapshot, and guidance prose MUST NOT become executable authority.

#### Scenario: Finite verify pilot

r[kiln.selected_work.completion.scenario.pilot]
- GIVEN the finite `verify` obligation definition
- WHEN Kiln evaluates passing, missing-check, stale-subject, changed-policy and interrupted-observation cases
- THEN only the passing case MUST complete and every other case MUST name its remaining obligation and owner

#### Scenario: Surfaces agree

r[kiln.selected_work.completion.scenario.surfaces]
- GIVEN one admitted snapshot bound to a campaign member
- WHEN worker context, campaign inspection and the UI projection are produced
- THEN they MUST carry the same obligation projection identity

### Requirement: Restricted display and legacy intake

r[kiln.selected_work.display] Kiln MUST keep audience-filtered display projections separate from complete trusted snapshots: a restricted display MUST NOT be admitted as a complete authoritative snapshot. The legacy readiness/intake reader MUST remain available only under an explicit compatibility profile.

#### Scenario: Display cannot be resubmitted

r[kiln.selected_work.display.scenario.resubmit]
- GIVEN a restricted display projection of an admitted snapshot
- WHEN its bytes are submitted for admission
- THEN admission MUST refuse it

### Requirement: Lattice workflow intent consumer

r[kiln.selected_work.lattice] Kiln MUST host-normalize an admitted snapshot's contributing obligations into a `lattice.workflow-intent.v1` input with the producer cohort identities, admit Lattice's compiled plan identity together with the selected obligations and attempt, and refuse agent-proposed workflow changes as executable until explicitly re-admitted with new exact identities. Deterministic fixtures MUST NOT be presented as live qualification.

#### Scenario: Proposal requires readmission

r[kiln.selected_work.lattice.scenario.proposal]
- GIVEN an admitted compiled plan for selected obligations
- WHEN a worker proposes a different intent or plan identity
- THEN Kiln MUST refuse it until the owner re-admits new identities

### Requirement: Typed member stages

r[kiln.selected_work.stages] Under the typed intake profile, Kiln MUST derive a pre-attempt member's stage, blockers and enumeration from its admitted snapshot's native phase readiness, stable blockers, unit states and dependency facts rather than from the legacy projections. An unobserved, unavailable or incompletely enumerated snapshot, a held planning phase, a blocked unit or an unready prerequisite MUST NOT make the member ready. The legacy projections MUST remain authoritative only under the explicit compatibility profile, and in-flight attempts MUST NOT be rewritten.

#### Scenario: Typed readiness overrides legacy readiness

r[kiln.selected_work.stages.scenario.disagree]
- GIVEN legacy readiness and the admitted snapshot disagree for one member
- WHEN the campaign refreshes under the typed intake
- THEN the member stage MUST follow the snapshot, and an unavailable or incomplete snapshot MUST hold the member
