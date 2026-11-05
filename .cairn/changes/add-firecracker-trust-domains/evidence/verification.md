# Verification evidence

## Result

Firecracker runners now have a reviewed trust domain. Untrusted-capable runners
require one, and a domain cannot mix untrusted and workload-bearing runners.
The receipt records the selected domain.

## Evidence

- All 63 hosted library tests pass.
- Workspace `--all-targets` check passes.
- Typed hosted Nickel profiles typecheck.
- `cairn validate` reports zero issues.

## Non-claims

Static domains do not prove physical host or LLC separation. Cross-attempt
co-residency sets, core pinning, CAT/MBA and covert-channel measurement remain
open. The domain label grants no hostile-isolation claim.
