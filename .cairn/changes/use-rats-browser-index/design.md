# Design: Rats browser index

## Context

The current catalog shell already independently admits histories, embeds sandboxed reports and exchanges only inert URL-fragment state. Replace only its primary index presentation, not report admission or CI meaning.

## Decisions

### Decision: Reuse an actual pinned Rats table via Ratzilla

An isolated browser Cargo workspace uses Ratzilla 0.3.1 and the selected pristine Rats widget source. A Kiln-owned import wrapper disables terminal-only Ratatui defaults without editing upstream source. Source revision, selected-file identities and licenses are retained. Normal builds use committed inputs, never ../rats or SSH/private ratcore. Browser state transitions remain pure; DOM, input and WASM initialization live in the browser shell.

### Decision: Preserve one offline artifact and explicit accessible navigation

Generate deterministic bundled JavaScript and base64 WASM from a locked browser crate and embed both in catalog HTML. Runtime does not fetch modules, WASM, fonts, journals or assets. Browser failure is visibly reported and leaves accessible HTML navigation intact. The generated assets are the normal packaged runtime input and have an explicit locked regeneration/check rail. No changes to provider or core crates are required.

### Decision: Keep the catalog bridge inert and bounded

The client receives only admitted catalog labels and identities, capped at the existing 128 histories. Its selection event carries project/history identity only. The existing catalog shell verifies a selected pair exists before changing the visible sandboxed report. Existing URL fragments, per-history view state and missing-selection behavior remain authoritative; no implicit replacement of removed histories. Accessible HTML controls are a supported surface, not a compatibility shim.

## Risks / Trade-offs

WASM increases HTML size, so it remains subject to rendered and publication bounds. Ratatui uses cell layout rather than native semantic tables; accessible HTML navigation and report text remain available. Selected widgets and native-only defaults must be exercised on wasm32, not assumed from version ranges. Existing independent Patchgate and release holds remain unchanged.
