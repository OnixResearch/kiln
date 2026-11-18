## Phase 1: Plan and eager local materializer

- [x] [serial] Add the pure materialization plan with input and overlay identities and refusals, with positive and negative tests. r[kiln.source.projection.materialization] r[kiln.source.projection.materialization.scenario.refused]
- [x] [serial] Add the eager local worker materializer with verified immutable inputs, private overlays and binding-guarded recovery, with tests over a temporary Git repository. r[kiln.source.projection.materialization.scenario.repeat] r[kiln.source.projection.materialization.scenario.isolated] r[kiln.source.projection.materialization.scenario.refused]

## Phase 2: Integration

- [x] [serial] Pin source, definition, manifest, materializer profile and overlay generation in dispatch and retained workspace metadata, refusing mismatches. r[kiln.source.projection.materialization] r[kiln.source.projection.materialization.scenario.pinned]
- [ ] [serial] Apply #69/#70 authority at retrieval and workspace effects and qualify the restricted runtime negatives and the two-project build profile from #77. r[kiln.source.projection.materialization]
