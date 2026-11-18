## Phase 1: Capture

- [x] [serial] Add the pure capture with typed edit, create and delete operations, its refusals, the sealed identity and reconstruction, with positive and negative tests. r[kiln.source.projection.writeback] r[kiln.source.projection.writeback.scenario.independent] r[kiln.source.projection.writeback.scenario.refused] r[kiln.source.projection.writeback.scenario.stale]

## Phase 2: Sealing

- [x] [serial] Add the worker shell that observes an overlay and seals a candidate commit from the base tree, with round-trip tests over a temporary Git repository. r[kiln.source.projection.writeback.scenario.round_trip] r[kiln.source.projection.writeback.scenario.sealed]
- [x] [serial] Document capture, sealing and their non-claims, and record verification evidence. r[kiln.source.projection.writeback]
