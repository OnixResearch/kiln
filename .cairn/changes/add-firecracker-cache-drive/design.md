## Context

The guest already extracts the read-only source ustar from `/dev/vda` into
tmpfs. A second read-only drive can carry a cache entry without a writable
host share.

## Decision

For Firecracker, restore the cache entry into a temporary host directory, build
a ustar image, attach it as `/dev/vdb`, and have the guest extract it after the
source. The source and cache image are separate files, so the image never
contains itself.

After a successful job with confirmed owned cleanup, the host requests the
declared output roots over the existing vsock session, bounds and validates the
returned ustar, extracts it into a private temporary directory, verifies the
cache key against the unchanged source workspace, and publishes only the
declared outputs. Guest writes never reach the host source workspace.

## Risks

- The cache entry must fit the configured source-image capacity.
- No object-storage tier or cache latency measurement.
- The host-level guest return path is exercised by the KVM cache publication
  journey; its result remains scoped to the recorded guest image and host.
