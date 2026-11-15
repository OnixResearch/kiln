## Phase 1: Publication boundary

- [x] [serial] Retain pending index publication, reuse exact durable one-file replacement and reconcile before acknowledgement without changing the content port API. r[kiln.aspen_runtime.content_index_retry]
- [x] [serial] Add real-port persistent failure/retry/reopen and populated-index regressions with positive and negative cases. r[kiln.aspen_runtime.content_index_retry.scenario.fault] r[kiln.aspen_runtime.content_index_retry.scenario.recovery] r[kiln.aspen_runtime.content_index_retry.scenario.preserved]

## Phase 2: Verification

- [x] [serial] Run focused runtime behavior and required applicable checks, retain exact dependency/profile/failure observations, and update the runtime docs/changelog and active traceability without archive/publication. r[kiln.aspen_runtime.content_index_retry] r[kiln.aspen_runtime.content_index_retry.scenario.barrier]
