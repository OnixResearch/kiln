## Phase 1: Native capability admission

- [x] [serial] Acquire exact Nanos/OPS and Unikraft source identities and compare native Firecracker support. r[kiln.hosted.unikernel.test]
- [x] [serial] Build and execute static Rust/Go pass/fail plus denied network probes under the selected native kernel. r[kiln.hosted.unikernel.test]

## Phase 2: Execution contract

- [x] [serial] Implement single ELF admission and fail-closed serial results with independent refusal regressions. r[kiln.hosted.unikernel.test]
- [x] [serial] Implement cold-only confined Firecracker dispatch and immutable image/unikernel receipts. r[kiln.hosted.unikernel.test]
- [x] [serial] Add reproducible flake image inputs and verify repeated BLAKE3 identities. r[kiln.hosted.unikernel.test]
- [x] [serial] Record measured cold boot, no-network observations and exact remaining prerequisites without qualification inflation. r[kiln.hosted.unikernel.test]
- [x] [serial] Pin a fixed guest-only nixpkgs and enforce the manifest kernel-version gate. r[kiln.hosted.guest.kernel.fixed]
- [x] [serial] Install and observe the substantive inherited preboot syscall policy; exercise strict Seccomp 2, three actual policy refusals and owned teardown before capture joins. r[kiln.hosted.unikernel.test]
- [x] [serial] Preserve read-only fixture permissions through RAMFS vnode recreation and prove real read-success/write-refusal in both native execution rails. r[kiln.hosted.unikernel.test]
