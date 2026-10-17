## Why

Kiln's existing workflow consumers need remote CI execution over authenticated Iroh peers without a second executor. The current consumer is the GitHub/Radicle Lattice workflow shell. Kiln maintainers own the adapter; operators retain peer enrollment, worker confinement, credentials and deployment authority.

## What Changes

- Add a versioned, Rust-1.88-compatible `kiln-adapter-iroh` shell pinned to Iroh 0.95.1 and ALPN `kiln.workflow.v1`.
- Carry the existing producer-owned Lattice exchange contract between a controller-local Unix proxy and a worker-local Lattice socket. Existing workflow consumers select the proxy socket through their existing configuration.
- Authenticate both endpoint keys, require explicit peer enrollment and exact contract-profile equality, and bound frames, logs, concurrent connections and exchange deadlines.
- Preserve dispatch, recovery, observation, controls, logs and reconciliation identities. A lost or malformed reply closes the local exchange, retaining unknown-after-write rather than permitting blind redispatch.
- Add typed Nickel configuration, a stable-key CLI, positive/negative conformance, actual two-endpoint execution evidence and operator documentation.

## Impact

- **Files**: new adapter crate, workspace manifest/lock, typed Nickel builder and fixtures, Nix app/check wiring, existing architecture/operator/README documentation and this lifecycle package.
- **Testing**: real loopback Iroh plus Unix Lattice exchanges; denied peers, stale profiles/identities, oversized/malformed frames, provider failure and timeout/recovery; exact-source local Lattice process smoke; root tests, strict Clippy, formatting, Nickel and native Cairn gates.
- **Non-goals**: read-only observation changes, new CI semantics or executor, arbitrary tunneling, remote publication, global exactly-once execution, sandboxing, provider correctness or release authority. Local transport evidence does not prove NAT/relay deployment or WAN availability.
