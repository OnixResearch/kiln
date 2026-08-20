# Tasks

- [x] [serial] Pin the exact Lattice transport and recovery cohort in Cargo, Nickel, fixtures, and documentation. r[kiln.lattice.transport_client]
- [x] [serial] Add `RecoverDispatch` to the application-owned exchange operation set. r[kiln.lattice.transport_recovery]
- [x] [serial] Implement the concrete bounded Unix workflow exchange client. r[kiln.lattice.transport_client]
- [x] [serial] Map every producer success, rejection, and client failure to explicit Kiln outcomes. r[kiln.lattice.transport_client]
- [x] [serial] Recover unknown dispatch acceptance without automatic redispatch. r[kiln.lattice.transport_recovery]
- [x] [parallel] Add positive local socket tests for dispatch, observe, control, logs, reconciliation, and recovery. r[kiln.lattice.transport_conformance]
- [x] [parallel] Add negative unavailable, disconnect, not-found, malformed, oversized, and stale tests. r[kiln.lattice.transport_conformance]
- [x] [serial] Run workspace, Clippy, wasm, Nickel, Cairn, Tracey, and Nix checks. r[kiln.lattice.transport_conformance]
- [x] [serial] Record evidence, sync the accepted spec, archive the change, and publish it. r[kiln.lattice.transport_conformance]
