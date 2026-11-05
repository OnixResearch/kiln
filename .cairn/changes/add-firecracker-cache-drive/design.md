## Context

The guest already extracts the read-only source ustar from `/dev/vda` into
tmpfs. A second read-only drive can carry a cache entry without a writable
host share.

## Decision

For Firecracker, restore the cache entry into a temporary host directory, build
a ustar image, attach it as `/dev/vdb`, and have the guest extract it after the
source. The source and cache image are separate files, so the image never
contains itself.

Publication is skipped because guest writes stay in tmpfs; caching the host
workspace would publish unchanged source.

## Risks

- Only restore is implemented.
- No object-storage tier or cache latency measurement.
- The cache entry must fit the configured source-image capacity.
