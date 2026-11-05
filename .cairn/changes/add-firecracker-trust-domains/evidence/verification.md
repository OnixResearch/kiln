# Verification evidence

## Result

Firecracker runners now have a reviewed trust domain. Untrusted-capable runners
require one, and a domain cannot mix untrusted and workload-bearing runners.
The receipt records the selected domain, host identity and the other same-attempt
jobs with the same declared host. A host cannot mix untrusted and workload-bearing
runners.

## Evidence

- All 63 hosted library tests pass.
- Workspace `--all-targets` check passes.
- Typed hosted Nickel profiles typecheck.
- `cairn validate` reports zero issues.

## Non-claims

Static domains and host labels do not prove live physical co-residency, host or
LLC separation. Cross-attempt
co-residency sets, core pinning, CAT/MBA and covert-channel measurement remain
open. The domain label grants no hostile-isolation claim.
