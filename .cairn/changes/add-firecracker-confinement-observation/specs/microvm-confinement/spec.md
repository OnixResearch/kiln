# MicroVM Confinement Specification

## Purpose

Defines host-observed Firecracker VMM confinement before a guest session.

## ADDED Requirements

### Requirement: Bounded process admission

r[kiln.microvm.confinement] The backend MUST parse the selected
`/proc/<pid>/status` fields with explicit byte, line and value bounds. It MUST
require exactly one `firecracker` process in the attempt unit cgroup, the
service user's UID/GID, `CapEff` zero, `NoNewPrivs` one, `Seccomp` two, and an
executable path equal to the reviewed immutable Firecracker path. Missing,
duplicated, malformed, oversized or unavailable evidence MUST fail closed.

#### Scenario: Confined VMM

- GIVEN exactly one reviewed Firecracker process in the attempt unit
- WHEN its status and executable are observed
- THEN the facts MUST be retained and the guest MAY connect.

#### Scenario: Changed or incomplete process evidence

- GIVEN extra processes, a different name, non-zero capabilities, seccomp below
  two, no-new-privileges off, a different executable or an unreadable process
- WHEN observation runs
- THEN the observation MUST be `ok: false` and no guest session MAY start.

### Requirement: Confinement lifecycle gate

r[kiln.microvm.confinement.lifecycle] `Spawned` MUST request a confinement
observation. `GuestConnected` MUST be illegal until
`ConfinementObserved { ok: true }`. A failed observation MUST journal and fail
the job with normal teardown.

#### Scenario: Successful observation

- GIVEN a spawned VMM and a passing observation
- WHEN the lifecycle advances
- THEN the next effect MUST be guest acceptance.

#### Scenario: Failed observation

- GIVEN a spawned VMM and a failed or unavailable observation
- WHEN the lifecycle advances
- THEN the job MUST collect and journal failure before destruction.

### Requirement: Host observation

r[kiln.microvm.confinement.observation] The shell MUST add
`NoNewPrivileges=yes` to the VMM unit, resolve the attempt unit's cgroup, and
observe the single VMM process before accepting the guest. For a warm restore
the observation MUST follow `/snapshot/load`; for a cold boot it MUST follow
spawn. An unavailable cgroup, multiple processes, unreadable status or a changed
executable MUST fail closed.

#### Scenario: Cold boot

- GIVEN a cold-boot Firecracker unit
- WHEN the VMM has spawned
- THEN the shell MUST observe confinement before accepting the guest.

#### Scenario: Warm restore

- GIVEN a warm-pool snapshot restore
- WHEN `/snapshot/load` succeeds
- THEN the shell MUST observe the restored VMM before accepting the guest.

### Requirement: Observed receipt evidence

r[kiln.microvm.confinement.receipt] The VM receipt MUST record the observed
confinement fields, the reviewed executable BLAKE3 identity and the observation
outcome. It MUST retain `jailer: false` and `hostile_isolation_claim: false`.
Observed fields MUST remain outside `canonical_outcome`.

#### Scenario: Cold boot and warm restore

- GIVEN a cold-boot or warm-restore Firecracker job
- WHEN the guest session is admitted
- THEN the receipt MUST contain the observed process facts for that path.

### Requirement: Explicit non-claims

r[kiln.microvm.confinement.nonclaims] Documentation MUST state that this
observes an existing unprivileged-VMM claim; it does not add the jailer, a guest
network path, root privileges or a hostile multi-tenant isolation claim.

#### Scenario: Local qualification passes

- GIVEN a retained observation on one host
- WHEN evidence is recorded
- THEN it MUST NOT generalize beyond that host and VMM cohort.
