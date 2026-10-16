## Why

Kiln publishes an Aspen Unix client and a hosted extension, but no process serves the client protocol. The current cross-repository harness imports Aspen test support and cannot be deployed.

The hosted extension also receives only a provider output reference. It must classify the provider result as `Unknown` because exact terminal bytes are absent.

A private Onix Core canary needs a Kiln-owned composition root that uses Aspen's real host and Lattice's real workflow exchange.

## What Changes

- Add `kiln-aspen-host`, a bounded Unix service for the Kiln Aspen client protocol.
- Pin and consume the reviewed Aspen runtime contract.
- Compose Aspen's native host around the exact `kiln-aspen-extension` executable.
- Route approved workflow effects through the pinned Lattice Unix adapter.
- Materialize one typed provider completion with the exact Lattice observation.
- Derive Kiln terminal meaning from that admitted observation.
- Keep disconnect-after-acceptance as `Unknown` and provide reconciliation.
- Add positive and negative host, protocol, effect, and process tests.

## Impact

- **Crates:** one new composition-root crate plus narrow protocol changes in `kiln-adapter-aspen` and `kiln-aspen-extension`.
- **Dependencies:** Kiln gains one immutable Aspen source dependency for the host binary only.
- **Compatibility:** the Aspen cohort moves to the exact materialized completion record. Legacy completion records fail closed.
- **Risk:** the first host uses process-local callback values. It cannot claim active-operation recovery after service loss.
- **Non-goals:** this change does not deploy a machine, replace Lattice, add fallback, or grant Kiln deployment authority.
