## Context

Blacksmith's public console reference at https://www.blacksmith.sh/ and its linked dashboard image use off-white, black, acid-yellow, hard borders and a dotted technical texture. Kiln's live shell extends catalog.css; the offline catalog is a separate surface and is not in scope.

## Decisions

### Decision: Live-only visual tokens

Define light/dark palette overrides in web.css, preserving the existing semantic success/error tokens. Add a separate --signal / --signal-text pair for the yellow primary action and neutral selection, plus --panel for existing interaction surfaces. Keep all three appearance modes and compact density. Use existing system sans and monospace stacks without downloading fonts.

### Decision: Framed console components

Use a compact icon-labelled sidebar, square Kiln mark, crisp separators, restrained hard shadows, monospaced micro-labels and tabular numeric observations. Work lists, filters, planning tabs and dialogs share the same tokens; data/status meaning remains explicit. Decorative icons are aria-hidden. Retain navigation labels, IDs, data attributes, focus, confirmation and keyboard contracts.

## Risks / Trade-offs

Yellow must not become a success/error signal; dark mode needs readable on-signal text. Dense controls must grow back to 44px on mobile, menus stay inside 320px, long identifiers wrap or scroll locally, and forced-color focus stays visible. Visual checking must cover non-empty real CI data, not just the empty issue page. Browser asset overrides establish candidate appearance only, not deployed executable bytes.

## Verification Strategy

Capture before/after desktop and 320px screenshots; exercise issue capture without submission, planning categories, filters, keyboard palette, retained run/log inspection, light/dark/system and compact density. Observe overflow, touch targets, disabled and focused controls. Run the applicable repository checks after all edits integrate. Retain bounded evidence and existing lifecycle/release holds; no production mutations or deployment.
