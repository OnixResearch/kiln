# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Firecracker host preflight facts

r[kiln.hosted.microvm.host_facts] The Firecracker core MUST represent reviewed
host kernel, SMT, KSM, MPTCP and resctrl observations and classify
CVE-2026-98432 from an explicit fixed-release table plus reviewed vendor package
backports. An untrusted-capable runner MUST refuse admission when the CVE is
vulnerable or unknown, SMT is active, KSM is running, or MPTCP listeners remain
after the sysctl is disabled, unless a non-empty operator override is supplied.
The classification and refusal MUST NOT claim a patched kernel, measured
isolation, or release authority.

#### Scenario: Fixed host

- GIVEN a fixed release or a reviewed vendor backport and safe SMT/KSM/MPTCP
  facts
- WHEN untrusted admission runs
- THEN admission succeeds without an override.

#### Scenario: Unsafe host

- GIVEN a vulnerable/unknown release, active SMT, running KSM, or residual MPTCP
  listeners
- WHEN untrusted admission runs without an override
- THEN admission fails with the specific reason.

#### Scenario: Explicit override

- GIVEN an unsafe fact and a non-empty operator override
- WHEN untrusted admission runs
- THEN admission succeeds and the caller is responsible for recording the
  override.
