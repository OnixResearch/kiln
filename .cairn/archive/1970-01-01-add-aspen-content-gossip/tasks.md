## Phase 1: Content

- [x] [serial] Open the host content store and publish exact effect output bytes before success. r[kiln.aspen_runtime.content_publication]
- [x] [serial] Verify exact round trips, reopen, bounds and live Iroh publication. r[kiln.aspen_runtime.content_publication]

## Phase 2: Gossip hints

- [x] [serial] Add versioned hints, profile bounds, topic derivation and dedupe. r[kiln.aspen_runtime.gossip]
- [x] [serial] Refuse malformed, expired and oversized hints. r[kiln.aspen_runtime.gossip.scenario.bounds]
- [x] [serial] Deliver one bounded hint over loopback Iroh without granting authority. r[kiln.aspen_runtime.gossip.scenario.live]
