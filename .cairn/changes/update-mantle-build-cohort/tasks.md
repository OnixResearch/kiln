# Tasks

## Phase 1: Cohort selection and identity

- [x] [serial] Record the currently pinned Mantle revision, executable identity, contract bytes, and fixture bytes as the baseline; confirm the published `main` revision and that the producer contract bytes are unchanged. r[kiln.mantle.cohort_refresh] r[kiln.mantle.cohort_refresh.contract]
- [x] [serial] Build the selected revision from a clean `git archive` export and record the executable path, BLAKE3 identity, and product version. r[kiln.mantle.cohort_refresh.identity]

## Phase 2: Binding updates

- [x] [serial] Update the Rust profile constants and the typed Nickel profile with the new revision and executable identity, keeping every other bound value unchanged. r[kiln.mantle.cohort_refresh] r[kiln.mantle.cohort_refresh.identity]
- [x] [serial] Regenerate the vendored source record and fixture source record for the new revision, and update the importer pin without touching historical receipts. r[kiln.mantle.cohort_refresh.records] r[kiln.mantle.cohort_refresh.drift]
- [x] [serial] Update the stack profile, Nix cohort check, and contributor documentation so every reference names the same revision. r[kiln.mantle.cohort_refresh.records] r[kiln.mantle.cohort_refresh.drift]

## Phase 3: Verification

- [x] [serial] Exercise the adapter tests, the independent producer-fixture consumer, and the negative profile and fixture cases. r[kiln.mantle.cohort_refresh.contract] r[kiln.mantle.cohort_refresh.drift]
- [x] [serial] Exercise Nickel checks, the `mantle-build-cohort` Nix check, workspace Clippy, and Cairn validation plus this change's proposal/design/tasks gates; record exact commands and outcomes. r[kiln.mantle.cohort_refresh] r[kiln.mantle.cohort_refresh.records]
