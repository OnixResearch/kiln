# Verification evidence

## Result

Firecracker runners can declare a reviewed cache-partition label. Admission
rejects a label shared by untrusted-capable and workload-bearing runners, and
the receipt records `vm.cache_partition`.

## Evidence

- Hosted library tests: 70 passed.
- Workspace `--all-targets`: pass.
- Hosted Nickel typecheck: pass.
- `cairn validate`: zero issues.

## Non-claims

No resctrl/CAT/MBA programming, LLC or cache-allocation proof, or covert-channel
measurement exists. The label depends on truthful operator configuration.
