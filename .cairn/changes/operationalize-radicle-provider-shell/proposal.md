# Operationalize the Radicle provider shell

## Why

Kiln publishes Radicle broker-compatible messages, but the binary uses `LocalFixtureService`. That fixture returns success without a real Kiln run or provider observation.

Radicle is Kiln's canonical repository host. Kiln needs a concrete shell for local Radicle source and trigger operations.

Status publication and collaboration mutation moved to the dedicated changes `publish-radicle-ci-status` and `mutate-radicle-cobs-single-writer`. This change keeps source acquisition, trigger intake, the shared capability profiles, and conformance.

## What Changes

- Replace the fixture-only runtime path with an optional concrete Radicle shell.
- Keep source and trigger capabilities in separate versioned profiles.
- Bind source reads to exact repository, signed-reference, revision, and canonical tree identities.
- Bind trigger intake to exact broker event, repository, revision, and replay facts without inferring an actor signature.
- Preserve unknown outcomes after ambiguous transport or process failure.
- Add typed Nickel profiles, named bounds, reconciliation, and conformance fixtures.
- Keep `LocalFixtureService` only in test and example surfaces.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted Radicle CI compatibility contract.

## Impact

- **Adapters**: extends `kiln-adapter-radicle` with concrete shell ports.
- **Core**: adds only provider-neutral facts and pure admission decisions when required.
- **Configuration**: adds typed Radicle capability profiles and limits.
- **Testing**: adds local positive fixtures and negative authority, drift, replay, timeout, and reconciliation cases.
