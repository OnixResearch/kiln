## Phase 1: Configuration

- [x] [serial] Add runner CPU affinity and immutable taskset configuration. r[kiln.hosted.microvm.cpu_affinity]
- [x] [serial] Validate bounded, duplicate-free CPU lists and require taskset. r[kiln.hosted.microvm.cpu_affinity]

## Phase 2: Execution and evidence

- [x] [serial] Wrap the VMM command with taskset and record the affinity. r[kiln.hosted.microvm.cpu_affinity]
- [x] [serial] Add parser tests, Nickel checks, hosted tests and Cairn evidence. r[kiln.hosted.microvm.cpu_affinity]
