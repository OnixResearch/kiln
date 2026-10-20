## Phase 1: Specification

- [x] [serial] Create the focused OpenSpec package for verified JSON object significant-index discovery.

## Phase 2: Sibling verified-logic primitive

- [x] [serial] Add a small executable verified primitive in `/home/brittonr/git/verified-logic` that discovers or returns verified first/last significant decoded JSON object byte indexes.
- [x] [serial] Add sibling positive/negative tests and run the sibling repository's required verification for the changed module.
- [x] [serial] Commit and push the sibling verified-logic primitive.

## Phase 3: UCAN integration

- [x] [depends:Phase 2] Sync the vendored verified-logic snapshot into UCAN with `./scripts/sync-verified-logic.sh` and redact source repo metadata in summaries.
- [x] [depends:Phase 2] Replace UCAN-local JSON significant-index trust with the new verified bridge while preserving fail-closed parse behavior before serde.
- [x] [parallel] Update public verified bridge docs, coverage inventory, and guard checks for the new live-path JSON index-discovery seam.

## Phase 4: Verification and archive

- [x] [serial] Run UCAN validation: fmt, workspace tests, clippy, no-std guard, bridge guard, specific/all OpenSpec validation, `git diff --check`, and `nix flake check /home/brittonr/git/ucan`.
- [x] [serial] Archive the OpenSpec, commit, push, and verify clean local/tracking/live remote parity.
