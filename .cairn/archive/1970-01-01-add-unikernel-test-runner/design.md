## Context

The runner is a distinct prebuilt-binary job, not a new spelling for shell steps. Firecracker's no-device declaration and observed confinement remain authoritative. Runtime qualification requires actual static Rust and Go execution, not a fake guest or mocked result.

## Goals / Non-Goals

Use pinned native unikernel inputs, a deterministic read-only image, fixed arguments/fixtures, bounded merged serial output and exactly one valid exit frame. No shell, network, subprocess harness, writable output, warm pool, debug hold or hostile-isolation claim.

## Decisions

The bounded native spike selects Unikraft core `a4c494e80141bd402961c7d96150dd218c79db37`, app-elfloader `0cd7a2b2c4d5ece85647c0c9903b50426e849420` and libelf `c03d4ceb22a2b9e9799238f9c277a14284d313ad` on Firecracker 1.16.1. Static Rust musl PIE and external-link Go static PIE execute and emit native success/failure frames; default Go ET_EXEC and Rust static glibc are not the admitted ABI. The native kernel emits status at process termination; arbitrary application log text is not independent authority. Fail closed on missing/ambiguous status and collect through VMM termination. A reserved KILN_ARGV_END boundary prevents Firecracker-appended boot parameters from changing fixed application argv. Keep guest-only nixpkgs independent of the existing producer cohort pin.

Native Firecracker inherits the explicit `kiln.native.preboot.v1` default-deny
syscall policy before exec. Before `InstanceStart`, read the real unit's exact
allowlist and EPERM setting, bind both authored/exported BLAKE3 identities and
the running manager cohort, then require strict Seccomp 2 for main and worker
threads. Never use the Cloud Hypervisor unfiltered-main exception. On refusal,
stop the owning transient unit before joining launcher capture workers so its
inherited service pipes cannot obscure the original policy error.

Preserve declared read-only fixture permissions across RAMFS vnode recreation.
The pinned upstream lookup reset them to all-permissions; the local patch uses
the persisted mode instead. CPIO creation temporarily grants owner-write access
until final archive permissions are applied. A real read-success/write-refusal
regression failed before this patch and passed afterward in both execution rails.

## Risks / Trade-offs

Linux ABI and Go runtime compatibility must be demonstrated. Static ELF inspection alone cannot prove a binary never invokes exec or sockets; the native kernel must enforce unsupported operations. Do not expose an executable backend without its native dependency and result contract.
