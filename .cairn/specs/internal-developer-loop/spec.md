# Internal Developer Loop Specification

## Purpose

Defines the `internal-developer-loop` capability.

## Requirements

### Requirement: Real pipeline observations

r[kiln.web.developer_loop.pipeline]
The service MUST support a real multi-node dependency workflow with observed step/node timing, measured resource facts and retained structured reports. Unknown historical facts MUST remain unknown.

#### Scenario: Pipeline success and failure

- GIVEN a reviewed multi-node workflow
- WHEN actual commands execute
- THEN dependency ordering and failures are preserved and the inspector presents source-bound observed timing/resource/report facts rather than invented progress.

### Requirement: Worker execution contract

r[kiln.web.developer_loop.workers]
Workers MUST publish authenticated bounded liveness/resource observations distinct from configured admission slots. Selected isolation MUST fail closed when unavailable, preserve bounded cancellation/timeout teardown, and retain exact recovery identity.

#### Scenario: Stale and unavailable execution

- GIVEN a stale heartbeat or unavailable selected isolation backend
- WHEN execution admission is evaluated
- THEN stale observations do not claim online capacity and no unsandboxed fallback executes.

### Requirement: Shared cache

r[kiln.web.developer_loop.cache]
Workers MUST restore and publish bounded provider-backed shared cache entries with declared input/producer/platform keys, verified BLAKE3 content, safe paths, explicit retention policy and actual hit/miss/transfer observations.

#### Scenario: Warm and corrupt cache

- GIVEN an admitted cold run then a matching warm run
- WHEN the second restores the cache
- THEN actual transferred bytes and hit/miss facts are recorded; corrupt or unsafe content is rejected before restoration.

### Requirement: Developer CLI and dirty inputs

r[kiln.web.developer_loop.cli]
The developer CLI MUST submit, watch, read logs, cancel and retry real executions through authenticated bounded APIs. Explicit dirty-worktree submissions MUST have exact content identity and be distinguishable from committed-ref runs, without publishing source or disclosing credentials.

#### Scenario: Exact dirty input

- GIVEN modified local source and explicit dirty input selection
- WHEN submitted and executed
- THEN the run observes exactly the admitted snapshot, retains its provenance and identity, and does not report the original committed tree as the executed input. Invalid input or stale identities MUST fail before effects.

### Requirement: Reviewed worker data mount roots

r[kiln.web.developer_loop.data_mount_roots] A worker isolation policy MAY
declare a bounded set of roots for read-only data mounts. Omitted roots MUST
preserve the default `/srv` and `/var/lib/kiln` set. Explicit roots MUST be
absolute, canonical, unique, bounded, and within the reviewed `/srv`, `/home`,
`/mnt`, `/opt`, or `/var/lib/kiln` families. Every individual mount MUST remain
under a selected root, be a canonical directory, reject symlink resolution and
checkout overlap, and be bound read-only. Selecting a root MUST NOT add write,
network, environment, credential, or arbitrary-path authority.

#### Scenario: Extended root admits a reviewed target

- GIVEN an execution policy with an explicit reviewed root under `/home` and one
  canonical target directory beneath it
- WHEN isolation admission and command construction run
- THEN the target MUST be bound read-only and the checkout MUST retain its
  separate write authority.

#### Scenario: Foreign or malformed root

- GIVEN a root outside the reviewed families, a relative path, `/`, a duplicate,
  an empty list, or more than four roots
- WHEN policy admission runs
- THEN it MUST fail closed without falling back to account-authority execution.

### Requirement: Reviewed read-only worker data mounts

r[kiln.web.developer_loop.data_mounts] A worker isolation policy MAY admit a
bounded set of explicit read-only data roots only under reviewed host roots.
Each mount MUST be canonical, a directory, outside the writable checkout, and
bound read-only in the sandbox. The policy MUST reject foreign roots, duplicate
entries, paths with traversal components, symlink-resolved paths, unavailable
paths, and mounts that overlap the checkout. Data mounts MUST NOT add network,
environment, capability, credential, or write authority, and unavailable
selected isolation MUST NOT fall back to account-authority execution.

#### Scenario: Exact data root is admitted

- GIVEN a reviewed worker execution policy with one canonical directory under
  `/srv` or `/var/lib/kiln`
- WHEN isolation admission and command construction run
- THEN the directory MUST be bound read-only and execution MUST remain confined
  to the selected bubblewrap policy.

#### Scenario: Foreign, linked or overlapping path

- GIVEN a data root outside the reviewed roots, a symlink-resolved path, a
  missing path, a duplicate, or a path inside the writable checkout
- WHEN admission or execution runs
- THEN the policy MUST fail closed without exposing the host path or falling
  back to account authority.

#### Scenario: Data mount is used by a worker step

- GIVEN an admitted Patchgate worker step with a target inside the declared data
  root
- WHEN the step runs
- THEN the target MUST be available read-only while the checkout, output and
  temporary review home retain their declared write authority.
