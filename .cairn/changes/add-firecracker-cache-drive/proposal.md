## Why

Firecracker jobs had no cache path. A read-only cache drive gives them restore
without exposing a writable host share or inventing guest publication.

## What Changes

- Materialize a cache entry as a read-only ustar image.
- Attach it as a second virtio-blk drive (`/dev/vdb`).
- Extract it over `/work` in the guest before steps.
- Keep publication disabled for Firecracker; no host workspace writes.

## Impact

- **Files**: Firecracker plan, hosted execution, guest agent, docs.
- **Testing**: plan drive test and hosted unit tests.
- **Authority**: restore only; publication, object tier and cache measurement
  remain open.
