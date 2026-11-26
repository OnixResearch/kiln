## Context

`web/dashboard-index` already ships a browser WASM client: Rust compiled to `wasm32-unknown-unknown`, bound with a pinned `wasm-bindgen`, bundled with a pinned `esbuild`, committed as generated assets, and checked by `scripts/build-dashboard-index.py --check` in the flake. Herdr 0.9.3 `pane read --source visible --format ansi` returns the visible screen with escape sequences, and `pane layout --pane <id>` reports the pane's cell size. Rio (`raphamorim/rio`) ships `librio`, an embeddable terminal state engine whose PTY, image decode and C ABI are optional features, and `librio-wasm`, a wasm-bindgen JavaScript ABI over it.

## Decisions

### Decision: Rio's librio is the terminal state engine

**Choice:** Import Rio at `b13db0193c61c105b86e12b4ac6c6fc255cf0a36` through the pinned-source importer (`vendor/rio`, `vendor/rio.source.json`): `librio` and its in-repository dependencies only. Kiln-owned wrapper manifests under `web/herdr-viewer/rio` point at the pristine imported sources and omit upstream's PTY, image, glyph, renderer, window and clipboard features, so no PTY, image or glyph decode, or C ABI is built. Wrappers inside the viewer workspace also give the crates checkout-independent Cargo metadata hashes, which keeps the WASM reproducible between the dev shell and the Nix sandbox (path dependencies outside the workspace hash their absolute path). The viewer does not import `librio-wasm`: that ABI exports keyboard, paste, selection, scroll and link entry points, and a read-only viewer must not ship them. The viewer's core reads the same per-cell state `librio-wasm`'s `write_cells` packs (character, wide state, grapheme cluster, foreground, background, style flags).

**Rationale:** Rio is a maintained, MIT-licensed VT engine with grapheme clustering and the full SGR palette. A vendored, pristine import matches Kiln's source-acquisition rule (no Git dependencies in browser locks). Depending on `librio` directly keeps the shipped JavaScript surface to mount, render and dispose.

**Toolchain:** Rio's workspace declares `rust-version = "1.100.0"`, but no imported package inherits it and the selection builds with the browser toolchain already pinned for the dashboard index (Rust 1.90.0, available from the flake's `rust-overlay`; 1.100.0 is not). The viewer keeps its own `rust-toolchain.toml` with that pin. No upstream byte is rewritten.

### Decision: Parse escapes in WASM, never as HTML

**Choice:** Each snapshot is fed to a fresh, throwaway `librio` surface sized to the reported pane dimensions (at least as many rows as the snapshot has lines), followed by CAN and an end-of-synchronized-update so a clipped or hostile tail cannot hide the grid. The surface's delegate drops every host-bound event: replies meant for the child, clipboard writes, titles, bells, notifications and close requests. The core reads back only cell text and resolved colors; hyperlinks, selection and images are never read. A Kiln-owned palette resolves colors, so palette-change sequences do not apply. The browser shell paints cells on a 2D canvas with `fillText` and sets the canvas fallback content with `textContent`. No captured byte reaches the DOM as markup.

**Rationale:** Pane output is untrusted agent output. Interpreting it only as terminal state in a fresh surface keeps it out of HTML and script contexts and out of any later snapshot.

### Decision: Reuse the dashboard-index pipeline

**Choice:** Add `web/herdr-viewer` as its own locked workspace with the same pinned toolchain, `wasm-bindgen` 0.2.121 and `esbuild` 0.27.2 (both from the `dashboard-index` dev shell's nixpkgs). The shared `scripts/browser_wasm_assets.py` pipeline builds both clients; `scripts/build-dashboard-index.py` keeps its output byte-identical and `scripts/build-herdr-viewer.py` writes `herdr-viewer.js`, `herdr-viewer.wasm.b64` and `herdr-viewer-notices.txt` into `crates/kiln-web-service/src/generated/`. The `herdr-viewer-assets` flake check runs `--check`, the crate's tests, wasm32 clippy and rustfmt. The web service serves the three assets from its static table, and its CSP adds `'wasm-unsafe-eval'` so same-origin WebAssembly can compile; JavaScript `eval` stays refused.

**Rationale:** One convention for browser WASM. Generated assets keep the service build independent of browser tooling.

### Decision: Read-only capture with explicit bounds

**Choice:** ANSI capture is `format=ansi&source=visible` on the existing operator-only capture route, rechecked against the host's current inventory, bounded to at most 64 KiB and the existing line limit, and returned with the pane's layout size when Herdr reports one. The console loads the module only when an operator opens a viewer. Refresh is client-driven, runs only while the viewer is open and the page is visible, no faster than once every two seconds, and stops on a stale pane or failed read.

**Rationale:** Keeps the authority unchanged: viewing never grants input, prompt, or interrupt.

## Risks / Trade-offs

- WebAssembly or a 2D canvas can be unavailable; the plain-text capture remains the fallback.
- A snapshot is not a stream: fast-changing output can be missed between refreshes.
- A worst-case 64 KiB snapshot escapes to more JSON than the smallest Web bridge response cap; with that profile the viewer's read fails and refresh stops. The documented 1 MiB profile fits it.
- Rio's root manifest pins a newer toolchain than Kiln builds with; a future Rio revision that needs it must move the browser toolchain pin.
