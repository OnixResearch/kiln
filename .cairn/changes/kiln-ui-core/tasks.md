# Tasks: `kiln-ui-core`

- [x] Create `.cairn/changes/kiln-ui-core/` lifecycle package (`proposal.md`, `design.md`, `tasks.md`).
- [x] Add `crates/kiln-ui-core` to workspace `members` and `default-members` in `Cargo.toml`.
- [x] Implement `uri.rs` (typed `BufferUri`, `Perspective`, bounded parsing, URL round-trip).
- [x] Implement `safety.rs` (`FreshnessGuard`, 4-level `SafetyTier`, `Unknown` rerun blocker, `PendingMutation`, `BatchReceiptLedger`).
- [x] Implement `selection.rs` (`SelectionSet` + strict `N >= 2` whitelist).
- [x] Implement `keymap.rs` (3-mode `Mode`, `PendingChord`, canonical keymap, `which_key_entries`, `modeline_hints`).
- [x] Implement `layout.rs` (BSP `SplitTree`, responsive `ViewportTier`, `Layer1Dock`, `Layer2Overlay`, `TokenRole`, `WorkspaceFrame`).
- [x] Implement `lib.rs` (`WorkspaceState::reduce` + exhaustive positive and negative tests).
- [x] Verify `cargo test --locked -p kiln-ui-core`, `cargo check -p kiln-ui-core --no-default-features`, and `cargo clippy -p kiln-ui-core --all-targets`.
