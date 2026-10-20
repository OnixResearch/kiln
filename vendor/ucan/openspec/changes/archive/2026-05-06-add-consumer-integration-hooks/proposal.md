## Why

`ucan` is expected to be consumed by more than one application, starting with Aspen and Clankers. The current readiness gates correctly avoid claiming built-in remote DID resolution, proof transport, registry-backed revocation, or replay persistence. Without generic hook traits, each consumer must either duplicate verifier plumbing or pressure `ucan` to grow application-specific adapters.

## What Changes

- Add a generic consumer integration profile for application-owned infrastructure.
- Define public hook traits for DID/key resolution, proof-chain loading, revocation checking, and replay/nonce admission.
- Require deterministic fake/in-memory implementations and conformance tests that prove fail-closed behavior without network or storage dependencies.
- Extend readiness reporting so hook-backed surfaces can be labeled independently from concrete production backends.

## Out of Scope

- Aspen capability or RPC mapping.
- Clankers tool/session/permission mapping.
- Built-in network, IPFS, database, or registry clients.
- Non-Ed25519 production promotion unless covered by a separate algorithm-support change.
- Treating hook existence as production support for any concrete backend.

## Capabilities

### Modified Capabilities
- `library.production-readiness.integration-profile`: the profile gains reusable hook traits and conformance evidence for multiple consumers.
- `library.production-readiness.remote-did-gate`: remote/key resolution promotion can rely on a generic resolver seam without choosing a backend.
- `library.production-readiness.proof-transport-gate`: proof transport promotion can rely on a generic proof loader seam without choosing a transport.
- `library.production-readiness.revocation-registry-gate`: revocation promotion can rely on a generic checker seam without choosing a registry.
- `library.production-readiness.replay-persistence-gate`: replay promotion can rely on a generic replay admission seam without choosing persistence.

## Impact

- **Files**: expected Rust changes under `src/`, docs under `docs/`, readiness JSON/docs, tests/examples, and OpenSpec archive updates.
- **APIs**: new generic traits and structured error types; existing issue/parse/verify APIs should remain compatible.
- **Dependencies**: no new mandatory network/storage dependencies.
- **Testing**: targeted trait conformance tests, readiness check, library tests, clippy, OpenSpec validation, and full Nix flake check before archive.
