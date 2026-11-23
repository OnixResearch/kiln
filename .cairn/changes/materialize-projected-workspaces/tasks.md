## Phase 1: Plan and eager local materializer

- [x] [serial] Add the pure materialization plan with input and overlay identities and refusals, with positive and negative tests. r[kiln.source.projection.materialization] r[kiln.source.projection.materialization.scenario.refused]
- [x] [serial] Add the eager local worker materializer with verified immutable inputs, private overlays and binding-guarded recovery, with tests over a temporary Git repository. r[kiln.source.projection.materialization.scenario.repeat] r[kiln.source.projection.materialization.scenario.isolated] r[kiln.source.projection.materialization.scenario.refused]

## Phase 2: Integration

- [x] [serial] Pin source, definition, manifest, materializer profile and overlay generation in dispatch and retained workspace metadata, refusing mismatches. r[kiln.source.projection.materialization] r[kiln.source.projection.materialization.scenario.pinned]
- [x] [serial] Run a typed shared corpus through the eager local materializer for two attempts in a harness shared by an off-by-default example and a test, and freeze its receipt apart from timings. r[kiln.source.projection.materialization.scenario.baseline]
- [ ] [serial] Qualify the restricted runtime negatives and the two-project build profile from #77 (non-authority half, `run-projected-workspaces`). Reopened: its receipt there ran under the local-operator profile and is superseded; requalify on S1's verified profile at integration. r[kiln.source.projection.materialization]
- [ ] [serial] Apply #69/#70 authority at retrieval and workspace effects (S7 `authorize-projected-workspaces`). r[kiln.source.projection.materialization]
