## Phase 1: Repair

- [x] [serial] Record both license texts in the source record and extract them in the importer. a[retain-heartwood-license-texts.importer]
- [x] [serial] Run the importer against the pinned commit and retain the license texts in the vendored directory. a[retain-heartwood-license-texts.retained]

## Phase 2: Verification

- [x] [serial] Exercise idempotent refresh and tampered-digest refusal, check the license-map links and the vendored crate's tests, validate lifecycle gates and retain bounded evidence. a[retain-heartwood-license-texts.verification]

The vendored crate's tests ran through Cargo in the development shell. The
`heartwood-canonical` flake check was not completed; `evidence/verification.json`
lists it under `not_exercised`. This package is not synchronized or archived.
