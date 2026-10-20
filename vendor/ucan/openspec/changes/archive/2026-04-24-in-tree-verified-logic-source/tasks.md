## Phase 1: Decide and specify

- [x] Choose the in-tree strategy and record the rationale in proposal/design/spec artifacts ✅ 8m (started: 2026-04-24T00:24:57-04:00 → completed: 2026-04-24T00:32:57-04:00)

## Phase 2: Implement vendoring

- [x] Create `scripts/sync-verified-logic.sh` to import committed upstream source and write `vendor/verified-logic-revision.txt` ✅ 5m (started: 2026-04-24T00:37:20-04:00 → completed: 2026-04-24T00:42:20-04:00)
- [x] Vendor the `verified-logic` crate snapshot into `vendor/verified-logic` and record the imported upstream revision ✅ 1m (started: 2026-04-24T00:37:20-04:00 → completed: 2026-04-24T00:37:24-04:00)
- [x] Update `Cargo.toml` and `flake.nix` to build against the vendored path, removing the old `verified-logic-src` input, combined copy step, and `postPatch` symlink shim ✅ 1m (started: 2026-04-24T00:37:40-04:00 → completed: 2026-04-24T00:38:05-04:00)
- [x] Document the refresh workflow and contributor expectations in `README.md` and `AGENTS.md` ✅ 1m (started: 2026-04-24T00:37:40-04:00 → completed: 2026-04-24T00:38:05-04:00)

## Phase 3: Verify

- [x] Run `scripts/sync-verified-logic.sh` against the sibling repo, confirm it refreshes the vendored snapshot and recorded revision, and keep the final success evidence ✅ 1m (started: 2026-04-24T00:37:20-04:00 → completed: 2026-04-24T00:37:24-04:00)
- [x] From `/tmp`, run `cargo metadata --format-version 1 --no-deps` and confirm `verified-logic` resolves to `vendor/verified-logic`; also confirm `Cargo.toml` and `flake.nix` contain no `../verified-logic` or `git+file:` references so fresh clones do not depend on a sibling checkout ✅ 24m (started: 2026-04-24T00:38:32-04:00 → completed: 2026-04-24T01:02:33-04:00)
- [x] Run `nix flake check --no-build` and confirm exit 0 with no relative `git+file:` warning, keeping the final success evidence ✅ 1m (started: 2026-04-24T01:14:53-04:00 → completed: 2026-04-24T01:14:54-04:00)
- [x] Run `nix develop <repo-root> -c cargo test --manifest-path <repo-root>/Cargo.toml --workspace` and confirm exit 0 with no relative `git+file:` warning, keeping the final success evidence ✅ 1m (started: 2026-04-24T01:15:38-04:00 → completed: 2026-04-24T01:16:39-04:00)
- [x] Run full `nix flake check` from `/tmp` with an explicit flake path and `DIRENV_*` unset, then confirm exit 0 with no relative `git+file:` warning ✅ 1m (started: 2026-04-24T10:59:00-04:00 → completed: 2026-04-24T10:59:07-04:00)
