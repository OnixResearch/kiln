## Why

The requested audit follow-up must harden real authentication and local host boundaries without retaining speculative changes. The deployed Celld Worker already checks a five-minute timestamp window and persists mutation nonces; the newly added in-memory nonce set is unbounded and does not protect the production server. Public hashes and public operator keys do not require secret-dependent timing protection.

## What Changes

- Remove the unbounded model-only nonce cache and unnecessary public-hash/seed comparison changes.
- Use the platform HMAC verification primitive in the deployed Celld Worker and exercise its actual freshness, signature, and durable nonce boundaries.
- Retain owner-only Aspen host socket permissions and document the same-user deployment requirement.
- Apply compatible Cargo security updates without changing the supported Rust minimum, imported source snapshots, or independently pinned runtime cohorts.

## Impact

- **Files**: Celld Worker/model and conformance fixtures; root Cargo manifests/lock where compatible updates exist; Aspen host socket fixture; existing authentication, runtime, and dependency documentation.
- **Testing**: Node SQLite Worker conformance; focused Rust authentication/provider tests; actual socket-mode smoke; root formatting, clippy, workspace, Cairn, Octet, and Nix gates. Dependency advisory counts are lockfile evidence, not proof of exploitability or release qualification.
- **Owner and consumer**: Kiln maintainers own repeatable hardening for the Celld EventStorePort, standalone Aspen host clients, and root workspace consumers. No publication, deployment, signing, or remote mutation is authorized.
