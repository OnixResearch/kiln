# Add Aspen-host content publication and bounded gossip hints

## Why

The Aspen host effect path produced content references without retaining exact
bytes in the host's Molten content store, and there was no bounded way to
announce content/state availability without implying authority. Issue #54
requires Aspen/Molten content contracts and non-authoritative discovery hints.

## What Changes

- Open a bounded Molten content store beside the host state root and publish
  successful effect output bytes before returning; publication failure fails the
  effect.
- Add versioned `GossipHint`, bounded `HintBounds`/`GossipProfile`, topic
  derivation, deduplication and a bounded live Iroh listener.
- Keep hints explicitly non-authoritative and content optional.
- Document the content store, gossip bounds and non-claims; add unit and live
  loopback Iroh tests.

## Impact

- **Files**: `runtime/kiln-aspen-host/src/content.rs`, `gossip.rs`,
  `effect.rs`, `effect/port.rs`, `main.rs`, runtime README and specification.
- **Testing**: content round trip/reopen/live Iroh/bounds; gossip encoding,
  expiry, oversize, dedupe and live loopback delivery.
- **Authority**: content storage and hint transport only. No reservation,
  authority grant, delivery proof, migration transport or execution request.
