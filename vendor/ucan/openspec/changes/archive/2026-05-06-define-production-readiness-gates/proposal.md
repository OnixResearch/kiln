## Why

The current UCAN crate passes its local verification gate and is suitable as a bounded Rust library for controlled integrations, but the README intentionally leaves several production platform surfaces caller-owned: remote DID resolution, proof transport, registry-backed revocation, replay/persistence, non-Ed25519 key types, and broader caveat semantics. Without explicit promotion gates, consumers can overread the green `nix flake check` result as a turnkey production auth platform.

## What Changes

- **Support levels**: Define operator-facing labels that distinguish the current controlled-integration library surface from future production-supported platform integrations.
- **Production integration profile**: Specify the first end-to-end deployment profile that wires resolver, proof-store, revocation, key-custody, and readiness evidence without making those backends implicit globals.
- **Promotion gates**: Add explicit gates for remote DID resolution, proof transport, registry revocation, replay/persistence, expanded algorithms, and caveat semantics before any of those surfaces can be documented as production-supported.
- **Aggregate readiness check**: Require a deterministic report/check that prevents README/API docs from claiming broader support than evidence proves.

## Capabilities

### New Capabilities

- `library.production-readiness.support-levels`: Consumers can tell which UCAN surfaces are controlled-integration ready, preview, or production-supported.
- `library.production-readiness.integration-profile`: Applications get a concrete production integration pattern without hidden network/storage/clock defaults.
- `library.production-readiness.promotion-gates`: Future feature work has evidence requirements before support labels can advance.

### Modified Capabilities

- `library.consumer-onboarding`: Support-limit docs must agree with readiness labels and the aggregate readiness report.

## Impact

- **Files**: `README.md`, `docs/public-api.md`, `docs/verified-boundaries.md`, likely new readiness docs/scripts/tests under `docs/` and `scripts/`, and implementation modules for selected integration adapters or examples.
- **APIs**: May add readiness-report types, explicit integration-profile examples, and adapter traits/fixtures; must not add implicit global resolver/proof/revocation backends.
- **Dependencies**: Any backend/example dependencies must remain optional and justified by profile evidence.
- **Testing**: `nix flake check`, targeted unit/integration tests, negative backend fixtures, readiness-report checks, and docs/status consistency checks.
