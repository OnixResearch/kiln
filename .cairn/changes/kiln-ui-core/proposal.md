# Proposal: `kiln-ui-core` — Pure `no_std + alloc` Helix/Emacs Control Plane UI Core

## Motivation

Kiln's frozen vanilla-JS console (`crates/kiln-web-service/src/*.js`) behaves like a traditional siloed web dashboard (~250px stacked chrome, blocking `inert` overlays, single-view pages). As Kiln transitions to the `#67`/`#68` Rats/Meteorite target architecture — and future native `kiln tui` / GPUI consumers — the operator interface should become a **Helix/Emacs-grade instrument** for the AI Software Factory control plane:

- Typed, deep-linkable `kiln://` buffers instead of siloed web pages.
- Helix-style selection-first `noun -> verb` grammar with strict multi-select safety whitelists.
- 3 discoverable modes (`Normal`, `Input`, `Command`) with live `which-key` popups and modeline top-3 hints.
- Responsive BSP tiling splits (2-pane Master+Detail default on 14" laptops, 3–4 pane tiling on `≥1500px`, single scrollable stack `<1024px`).
- All Kiln safety invariants (30s per-buffer freshness gate, fail-closed `Unknown` run rerun block, exact BLAKE3/fence binding, no optimistic auto-advance, no blind multi-agent broadcast) enforced inside one pure `no_std + alloc` Rust state machine.

## Scope

1. Introduce `crates/kiln-ui-core` (`#![no_std]` with `extern crate alloc`, zero I/O, zero DOM/terminal deps, zero provider imports).
2. Implement canonical `BufferUri`, 3-mode browser-safe keymap reducer, typed `SelectionSet` + batch safety whitelist + `BatchReceiptLedger`, 4-level safety classifier + `FreshnessGuard`, and responsive `SplitTree` + `WorkspaceFrame` visual IR.
3. Add positive and negative unit tests exercising every invariant without starting any external service.
