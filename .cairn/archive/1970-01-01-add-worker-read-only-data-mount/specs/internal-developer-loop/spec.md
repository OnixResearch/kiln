# Internal Developer Loop Specification

## ADDED Requirements

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
