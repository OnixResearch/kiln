# Design: `kiln-ui-core`

## Architecture

`kiln-ui-core` is a pure functional core (`#![no_std]`, `extern crate alloc`, `unsafe_code = "forbid"`). Thin imperative shells (Rats/Meteorite WASM `#67/#68`, Ratatui `kiln tui`, or future GPUI desktop) feed `UiEvent` values into `WorkspaceState::reduce(event, now_ms)` and execute the returned `Vec<UiEffect>` while rendering the projected `WorkspaceFrame`.

## Modules

- `uri.rs`: Canonical `BufferUri`, `ObjectKind`, `Perspective` (`Cockpit | Mosaic | Review | CiFleet`), strict bounded validation (rejecting control characters, invalid BLAKE3 hex, oversized segments), and bijective URL query serialization (`to_query_string` / `from_query_string`).
- `safety.rs`: Named bounds (`CONTROL_FRESHNESS_MS = 30_000`, `MAX_SELECTION_ITEMS = 64`), `FreshnessGuard`, `SafetyTier` (`Tier1Instant | Tier2aInput | Tier2bInlineConfirm | Tier3DestructiveOverlay`), `Verb`, `MachineHealth` (explicit `Unreachable | Stale | NoReport`), `RunOutcome` (`Unknown` blocks `Rerun`, requires `ProbeReconcile`), `PendingMutation` (no optimistic auto-advance), and `BatchReceiptLedger`.
- `selection.rs`: Typed `SelectionSet` carrying `(ObjectKind, identity, fence_or_version)` with strict `N >= 2` whitelist enforcement (`OpenSplits`, `Yank`, `PipeContext`, `RenewClaim30m` [auto-promoted to `Tier2b`], `ProbeReconcile` [auto-promoted to `Tier2b`]; forbids blind prompt broadcast, bulk cancel, bulk force-release, bulk enqueue).
- `keymap.rs`: 3-mode `Mode` (`Normal | Input | Command`), `PendingChord`, collision-free browser-safe canonical keymap (`Esc` unconditionally exits to `Normal`; `1`–`4` perspectives; `Tab`/`Shift-Tab` + `Space w` focus; `g c/l/a/p/r/d/q/m/s/[/]` exact graph jumps; `Space` live `which-key`; modeline top-3 legal hints).
- `layout.rs`: Binary BSP `SplitTree`, 3 responsive `ViewportTier`s (`Phone < 1024`, `Laptop 1024..1499`, `Wide >= 1500`), mutually exclusive `Layer1Dock`, `Layer2Overlay`, `TokenRole` (active focus uses `FocusInkBorder`, never `BrandIdentity`), and `WorkspaceFrame`.
