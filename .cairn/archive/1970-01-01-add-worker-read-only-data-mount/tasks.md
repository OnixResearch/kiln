## Phase 1: Policy admission

- [x] [serial] Add optional `read_only_data_mounts` to `IsolationPolicy` with bounded `/srv` and `/var/lib/kiln` admission. r[kiln.web.developer_loop.data_mounts]
- [x] [serial] Canonicalize, require a directory, reject symlink resolution and checkout-inside mounts, and bind retained paths read-only. r[kiln.web.developer_loop.data_mounts]
- [x] [serial] Add admission and command-argument tests, including duplicate, foreign-root and checkout-overlap rejection. r[kiln.web.developer_loop.data_mounts]

## Phase 2: Worker profile and documentation

- [x] [serial] Expose the optional field through `config/web-worker.ncl` and update the Patchgate worker fixture. r[kiln.web.developer_loop.data_mounts]
- [x] [serial] Document the read-only mount authority, narrow roots, target requirement and non-claims. r[kiln.web.developer_loop.data_mounts]
- [x] [serial] Run focused execution tests, worker profile export, Nickel check, formatting and lifecycle validation. r[kiln.web.developer_loop.data_mounts]
