# Tasks

## Phase 1: Generation binding

- [x] [serial] Bind operator-selected worker generation to executor job admission, existing capability and JWT claims; reject inactive, unknown or mismatched durable rows at every authority boundary. r[kiln.hosted.fleet.secret_generation]
- [x] [serial] Revoke matching jobs atomically on observed retirement, without falsely completing revocation on uncertain teardown. r[kiln.hosted.fleet.secret_generation]

## Phase 2: Qualification and evidence

- [x] [serial] Test current, stale, replaced, retired, unknown, expired, uncertain and reopened generations; extend the real-process fake-provider receipt with expiry/revocation checks. r[kiln.hosted.fleet.secret_generation]
- [x] [serial] Run focused tests, example tests, Clippy and rustfmt; retain verification and receipt evidence, and update hosted documentation and non-claims. r[kiln.hosted.fleet.secret_generation]
