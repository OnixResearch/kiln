## Phase 1: Admission

- [x] [serial] Pin the Rust-compatible Iroh cohort and implement pure profile/peer/native-contract admission with typed Nickel positive and negative inputs. r[kiln.iroh.workflow.profile]

## Phase 2: Transport

- [x] [serial] Implement authenticated worker serving and the private controller Unix proxy with bounded framing, deadlines, concurrency and owned shutdown. r[kiln.iroh.workflow.transport] r[kiln.iroh.workflow.peers]
- [x] [serial] Preserve every native workflow operation and identity, unknown-after-write classification and exact recovery without redispatch. r[kiln.iroh.workflow.recovery]
- [x] [parallel] Add stable protected key handling, usable CLI commands and Nix app wiring; document adoption through the existing consumer socket setting. r[kiln.iroh.workflow.profile] r[kiln.iroh.workflow.transport]

## Phase 3: Verification

- [x] [serial] Exercise positive and negative real-Iroh transport cases, malformed and oversized frames, denied peers/profile drift, provider failure, timeout and recovery. r[kiln.iroh.workflow.conformance]
- [x] [serial] Exercise an actual two-endpoint remote CI workflow through worker-local Lattice; run affected tests, strict Clippy, formatting, Nickel and Cairn gates and record scoped evidence. r[kiln.iroh.workflow.conformance] r[kiln.iroh.workflow.recovery]
