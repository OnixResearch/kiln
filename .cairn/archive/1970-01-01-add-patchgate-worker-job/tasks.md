## Phase 1: Run-bound job

- [x] [serial] Change the job script to invoke the adapter `run` command and read its atomically published receipt. r[kiln.patchgate.worker.job]
- [x] [serial] Add the typed `config/patchgate-review-job.ncl` worker-step builder with explicit executable, checkout, output, and report fields. r[kiln.patchgate.worker.job]
- [x] [serial] Add positive and negative Nickel fixtures for the builder. r[kiln.patchgate.worker.job]

## Phase 2: Verification and documentation

- [x] [serial] Update the operator procedure and focused Nickel check. r[kiln.patchgate.worker.job]
- [x] [serial] Run script syntax/help, Nickel export/negative/format, fake-native smoke, focused Rust tests, and lifecycle validation. r[kiln.patchgate.worker.job]

## Deferred

- Live Patchgate/Pijul deployment qualification is not part of this change.
