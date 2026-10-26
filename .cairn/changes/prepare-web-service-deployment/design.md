## Context

The current HTTPS service already reserves intent durably, reconciles original operations after restart and responds to SIGINT/SIGTERM. It must become an installable supervised service without replacing its canonical state or its provider ports. Current native cancellation is misclassified by Lattice; Kiln must not fix that by relabeling observations.

## Decisions

### Decision: Product-owned deployment assets over existing system mechanisms

**Choice:** Keep deployable Kiln assets and a dedicated package in Kiln, reuse OnixOS module conventions for target assembly and ordinary systemd supervision on Linux. Profiles remain typed Nickel exports; secrets are operator/artifact-managed runtime files, never store literals. A dedicated non-root account owns persistent state. Repositories are read-only to the server; only its declared state is writable. Worker workloads retain separate authority.

**Rationale:** Kiln owns its application contract, not host inventory or a new activation engine. Installation/configuration does not authorize activating an actual host.

### Decision: Explicit startup and readiness

**Choice:** Add `check --profile PATH` as non-mutating validation of the admitted runtime inputs. Notify systemd only after TLS, listener and canonical state are initialized; expose minimal same-origin HTTPS health without private run data. Invalid configuration and private-file admission fail before readiness. Shutdown continues to reconcile durable state, never treats service exit as cancellation of external work, and remains supervised by an explicit stop deadline.

**Rationale:** A running process is not sufficient startup evidence, and checking configuration must not initiate workflows or create canonical state.

### Decision: Private read-only credentials

**Choice:** Accept owned single-link regular credentials with owner-only read permissions (0400 or 0600). Also admit root-owned 0440 files only on a read-only mount with the exact Linux POSIX ACL granting read solely to root and the current non-root UID: no group, other-user or world grant. Continue no-follow admission and exact token format. Apply bounded private-file admission to TLS private keys. No credential bytes enter profiles or logs, and no privileged helper or runtime credential copy is added.

**Rationale:** The actual packaged VM showed systemd credentials are root-owned 0440 files with a per-service UID ACL, not service-owned 0400 files. Checking the exact ACL and read-only mount preserves isolation without admitting ordinary group-readable files or trusting a pathname/environment variable as proof of provenance.

### Decision: Fix cancellation in the producer

**Choice:** Lattice derives durable cancellation from observed owned runtime completion, not merely cancellation intent, with positive/negative and completion-race cases. Preserve existing consumer protocol identities. Qualify the corrected immutable producer with the strict Kiln native assertion unchanged.

**Rationale:** Consumer relabeling would hide provider behavior and cannot establish that work stopped.

## Risks / Trade-offs

- Reviewable commands execute with the worker account's authority; deployment hardening is not hostile-code confinement.
- Certificate/token rotation uses a controlled restart; profile/state identity is not silently migrated.
- Active external workflows outlive server restarts. Timeouts and ambiguous provider outcomes remain uncertain; no blind redispatch.
- Native producer build prerequisites remain explicit; an exact local build does not prove public anonymous acquisition.
- Local service-manager/VM qualification is scoped evidence, not production activation, lifecycle approval or release eligibility.
