# Verification evidence

## Result

Firecracker cache restore uses a read-only second drive. The guest extracts it
over `/work`; publication stays disabled.

## Evidence

- Plan cache-drive test: 1 passed.
- Hosted library tests: 71 passed.
- `cairn validate`: zero issues.

## Non-claims

No guest publication, object-storage tier, host-level cache-drive integration
test or latency measurement exists yet.
