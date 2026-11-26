## Why

The Fleet console shows Herdr pane captures as sanitized plain text. Colors, cursor position, and full-screen layouts from agents are lost, so operators cannot read a pane the way it looks in Herdr.

## What Changes

- Add a browser-side WebAssembly terminal viewer built on Rio's `librio` terminal state engine (a pinned, vendored source import), following the existing `web/dashboard-index` pipeline: a separate locked workspace, a build script that produces committed generated assets with license notices, and a freshness check.
- Fleet capture accepts an explicit `format=ansi` with `source=visible` and returns Herdr's ANSI snapshot bounded by bytes and lines, still operator-only and rechecked against the host's current inventory.
- The viewer parses the ANSI snapshot into a fixed cell grid inside WASM and paints the cells on a 2D canvas. It is read-only: no input is sent to the pane.
- An optional refresh re-reads the selected pane at a bounded interval while the viewer is open and visible. A stale or closed pane stops the refresh.
- Plain-text capture remains available when the module fails to load or WebAssembly or a 2D canvas is unavailable.

## Impact

`crates/kiln-fleet` capture route and Herdr adapter, `crates/kiln-web-service` bridge, served assets, content security policy and Fleet console, a new `web/herdr-viewer` workspace, a `vendor/rio` source import, a build script shared with the dashboard index, the flake freshness check, and docs.
