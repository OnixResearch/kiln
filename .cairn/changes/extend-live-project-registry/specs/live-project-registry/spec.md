## ADDED Requirements

### Requirement: Explicit append-only project extension

r[kiln.web.projects.extension]

The offline extension command MUST require the exact prior admitted profile and a strictly appended admitted project suffix, preserve existing project identities and every other authority/resource field, permit only an increased max_projects bound, acquire the existing state lock, validate existing versioned state and retained evidence, require terminal existing runs, and atomically compare-and-swap only the durable profile binding. It MUST NOT rewrite stored canonical state, perform provider effects, enroll projects implicitly, initialize missing state, or weaken ordinary exact-profile startup admission.

#### Scenario: Preserve existing history while adding projects

- GIVEN terminal retained runs and an exact old profile
- WHEN the operator extends the registry with a valid appended project suffix
- THEN the transition occurs once, preserves state bytes, logs, attempts and source identities, opens under the new profile, rejects old-profile startup, and exposes added projects as available but not enrolled.

#### Scenario: Reject authority drift and stale transition

- GIVEN an existing profile-bound service store
- WHEN an extension contains existing project changes/removal/reordering, changed credentials/service/state roots/non-project bounds, shrinking capacity, duplicate/invalid projects, a stale prior identity, replay, a held service lock, active execution, missing/corrupt state or transaction failure
- THEN it MUST fail without advancing the profile binding.

### Requirement: Source-bound OnixResearch project enrollment

r[kiln.web.projects.onixresearch]

The operator MUST select documented repository-local CI commands over actual committed refs, install reviewed typed profiles and exact Lattice routes, retain the existing Kiln cohort and history, and use authenticated enrollment/run intake. Live projects MUST expose real source-bound observations and retained output; failures and unavailable prerequisites MUST remain explicit.

#### Scenario: Additional projects are real and observable

- GIVEN compatible repositories with reviewed source-bound CI definitions
- WHEN the operator installs exact routes, extends the registry and enrolls/runs the projects
- THEN the repositories appear in live Projects and Workers views, each initial execution names its exact commit and real command outcome, and original Kiln runs and logs remain readable.

#### Scenario: Unavailable prerequisites are not invented

- GIVEN a repository with unavailable CI prerequisites
- WHEN the operator reviews its documented CI definition
- THEN the exact exclusion reason is recorded and no fake job or successful placeholder row is added.
