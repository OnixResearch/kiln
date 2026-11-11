## Context

The existing microVM core already owns pure confinement observation. Host
preflight facts fit the same boundary: the shell reads `/proc`, `/sys` and
`uname`; the core classifies and admits them without I/O.

## Decisions

### Checked-in fixed table

CVE-2026-98432 is classified from explicit `(major, minor, fixed_patch)`
entries for the fixed series. A release in a known series is vulnerable below
the fixed patch and fixed at or above it. Unknown series are `unknown`.

Vendor kernels backport fixes, so an exact package string may be supplied with
a reviewed `CveStatus` backport entry. This map is shell/profile input, not
inferred from the release string.

### Refusal and override

`HostFacts::untrusted_refusal` returns a reason for any unsafe fact. A
non-empty operator override admits the runner; the caller must record it.
MPTCP sysctl `0` is not treated as mitigation when listeners remain.

## Risks

- Backport entries are operator-reviewed claims, not kernel testing.
- Receipt and scheduler integration are not implemented by this change.
