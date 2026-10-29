# Use Rats widgets for the browser index

## Why

The catalog index is handwritten HTML while the operator requests actual reuse of Subwayrat widgets. Ratzilla supplies a maintained browser backend compatible with Rats' Ratatui 0.30 API.

## What Changes

- Make an actual Rats data table the primary browser repository/history index through Ratzilla and WebAssembly.
- Keep canonical admission, independently sandboxed reports, missing selections, saved per-history state and read-only semantics unchanged.
- Embed compiled JavaScript and WASM in the existing single offline HTML artifact; retain accessible HTML navigation and no-JavaScript reports.
- Pin and import only the required MIT Rats source, without sibling runtime paths or the unrelated private ratcore dependency.

## Impact

- Consumer: the existing Kiln catalog and Aspen3 offline report. Owner: Kiln dashboard maintainers own browser integration and generated assets; Subwayrat maintainers own imported widgets.
- Files: dashboard catalog rendering/assets, an isolated browser Rust crate, source importer/provenance, Nix packaging/checks, existing operator and licensing documentation.
- Adoption: ordinary catalog rendering embeds checked-in compiled assets; explicit generation/check commands reproduce them from locked sources.
- Verification: real browser keyboard/mouse selection, saved/missing selections, hostile labels, narrow viewport, file/offline loading, renderer bounds and packaged source checks. No CI or forge authority is added.
