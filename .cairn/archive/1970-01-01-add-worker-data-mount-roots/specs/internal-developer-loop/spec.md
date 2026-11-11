# Internal Developer Loop Specification

## ADDED Requirements

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
