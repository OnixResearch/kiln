# Verification evidence

## Result

The microVM core now has a pure host-preflight model. It classifies
CVE-2026-98432 from a checked-in fixed-release table plus reviewed vendor
backports, retains SMT/KSM/MPTCP/resctrl observations, and refuses untrusted
admission on unsafe or unknown facts unless a non-empty operator override is
supplied.

## Evidence

- All 33 `kiln-microvm` library tests pass.
- Formatting passes.
- `cairn validate --root .` reports zero issues.

## Non-claims

No host files were read and no receipt was wired in this change. Vendor
backport entries are reviewed claims, not kernel testing. Scheduler
trust-domain placement, core partitioning and covert-channel measurement remain
unimplemented.
