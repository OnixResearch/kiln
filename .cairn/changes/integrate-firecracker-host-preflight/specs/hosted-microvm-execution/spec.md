# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Firecracker host receipt and untrusted refusal

r[kiln.hosted.microvm.host_receipt] Every Firecracker attempt receipt MUST
include the observed host kernel/package, CVE status, SMT, KSM, MPTCP and
resctrl facts. An untrusted-capable runner MUST refuse dispatch on a vulnerable
or unknown CVE, active SMT, running KSM, or residual MPTCP listeners unless the
reviewed runner supplies a non-empty operator override. The receipt MUST retain
the observed facts regardless of override. This observation MUST NOT claim a
patched host, hostile isolation, scheduler placement, partitioning, or release
readiness.

#### Scenario: Safe host admits untrusted work

- GIVEN a fixed kernel and safe SMT/KSM/MPTCP facts
- WHEN an untrusted-capable runner dispatches
- THEN the job may start and the receipt carries the host facts.

#### Scenario: Unsafe host is refused

- GIVEN an unsafe fact and no override
- WHEN an untrusted run is admitted
- THEN dispatch fails before credentials or job effects.

#### Scenario: Override is recorded

- GIVEN an unsafe fact and a non-empty reviewed override
- WHEN the job starts
- THEN the receipt still records the unsafe observed facts and the override
  remains operator-owned authority.
