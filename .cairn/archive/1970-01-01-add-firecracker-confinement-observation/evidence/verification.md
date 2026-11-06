# Verification evidence

## Result

The Firecracker shell now observes the running VMM before admitting the guest.
The attempt unit adds `NoNewPrivileges=yes`; the shell resolves the unit cgroup,
requires exactly one `firecracker` process, reads its `/proc/<pid>/status` and
`/proc/<pid>/exe`, and admits the service UID/GID, zero `CapEff`, `NoNewPrivs`
one, `Seccomp` two and the reviewed executable path. A failed observation
journals failure and never reaches `GuestConnected`.

## Evidence

- `cargo test -p kiln-microvm`: 30 passed, including status-parser positive and
  negative cases and lifecycle ordering.
- `cargo test -p kiln-hosted --lib vmm_unit_sets_no_new_privileges`: pass.
- KVM-gated `cargo test -p kiln-hosted --test microvm -- --nocapture` with
  `KILN_MICROVM_REQUIRED=1`: cold boot, warm restore and held-job paths pass.
  Receipts report `ok: true`, `seccomp: 2`, `no_new_privs: 1`,
  `cap_eff: 0000000000000000`, the reviewed executable and its BLAKE3 identity.
- `cargo check -p kiln-hosted --all-targets` passes.

## Non-claims

This observes one VMM process on one host cohort. It does not prove the reviewed
Firecracker binary has no vulnerabilities, and it does not add the jailer, a
guest network, a root helper or a hostile multi-tenant isolation claim.
`jailer: false` and `hostile_isolation_claim: false` remain; confinement fields
stay outside `canonical_outcome`.
