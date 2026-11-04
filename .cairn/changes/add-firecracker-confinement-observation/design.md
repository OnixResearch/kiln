## Context

Firecracker runs unprivileged under `systemd-run --user`. The service does not
use the jailer because it needs root. The existing receipt has fixed
`network: none`, `jailer: false` and `hostile_isolation_claim: false` fields;
nothing reads the VMM process itself.

Issue #31 requires the observation after spawn and after a warm-pool
`/snapshot/load`, because Firecracker installs the main-thread seccomp filter at
different points on the cold and restore paths. The check must fail closed:
missing evidence is not a pass.

## Decisions

### Pure parser, host-owned I/O

Add `kiln-microvm/src/confinement.rs` with a bounded parser for
`/proc/<pid>/status` plus an explicit executable path. It performs no I/O and
returns structured facts or a stable rejection reason. `kiln-hosted` owns the
cgroup lookup, process read and executable comparison.

The parser requires:

- `Name: firecracker`;
- first `Uid:`/`Gid:` values equal to the service user;
- `CapEff: 0000000000000000`;
- `NoNewPrivs: 1`;
- `Seccomp: 2`;
- exactly one occurrence of each selected field;
- the `/proc/<pid>/exe` target equal to the reviewed executable path.

Unknown fields are ignored. Oversized, non-UTF-8, malformed, duplicated or
missing selected fields are rejected.

### Observed lifecycle event

Add `Event::ConfinementObserved { ok: bool }` and `Effect::ObserveConfinement`.
`Spawned` moves to `Starting` and requests the observation. `GuestConnected` is
illegal until a successful observation has been recorded. A failed observation
moves to `Collected` and requests journaling, exactly like a failed boot.

For a cold boot the shell observes immediately after spawn. For a warm restore
it loads the snapshot first, then observes, so the check covers the post-load
seccomp installation. The lifecycle sees one final observation; a pre-load
failure still fails closed because the guest never connects.

### Unit hardening

`unit_command` adds `NoNewPrivileges=yes` for the VMM unit. The property is
included in the generated `systemd-run` arguments and is covered by a unit test.
If the host user manager refuses the property, spawn fails before a guest
session.

### Receiver evidence

The `vm` receipt gains:

```json
{
  "confinement": {
    "ok": true,
    "name": "firecracker",
    "uid": 1000,
    "gid": 1000,
    "cap_eff": "0000000000000000",
    "no_new_privs": 1,
    "seccomp": 2,
    "exe": "/nix/store/.../bin/firecracker",
    "exe_blake3": "..."
  }
}
```

A failed observation records `{ "ok": false, "error": "..." }` and the job fails.
`jailer` stays `false` and `hostile_isolation_claim` stays `false`.

## Risks

- Firecracker may expose more than one process in the unit (for example a
  launcher). The check requires exactly one and fails closed; this is deliberate.
- The VMM can exit between `cgroup.procs` and `/proc` reads. That is an
  unavailable observation, not a pass.
- `NoNewPrivileges=yes` may be unsupported by some user managers. The unit then
  fails before any guest session; the operator must fix the host or use another
  admitted backend.
- The observed fields are not part of `canonical_outcome`, so CI parity is
  unchanged.
