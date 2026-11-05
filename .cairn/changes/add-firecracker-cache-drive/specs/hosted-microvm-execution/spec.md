# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Read-only Firecracker cache drive

r[kiln.hosted.microvm.cache_drive] A Firecracker cache policy MUST materialize
the restored cache entry as a read-only second virtio-blk drive and the guest
MUST extract it over `/work` after the source drive. The drive MUST NOT be
writable and MUST NOT be the source drive. Firecracker cache publication MUST
remain disabled until a guest return protocol exists.

#### Scenario: Cache image is read-only

- GIVEN a Firecracker runner with a cache policy and a hit
- WHEN the VM configuration is built
- THEN it has a second read-only `cache` drive.

#### Scenario: No publication

- GIVEN a successful Firecracker job with a cache restore
- WHEN the job finishes
- THEN no cache entry is published from the host workspace.
