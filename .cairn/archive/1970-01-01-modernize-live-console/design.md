## Context

The live console is served by `kiln-web-service` from embedded static assets: `web.html`, `web.css`, `web.js`, work assets and code-review assets, layered over the offline `catalog.css`. The accepted `kiln.web.visual_design` and `kiln.web.minimal_console` requirements fix the palette, routes, appearance modes, 44px mobile targets, 320px containment and keyboard contracts. An audit of the live service with real retained CI data found raw enum statuses, unformatted durations, absolute-only times, duplicate Refresh actions, primary-styled session controls and dense run-detail cards.

## Goals and Success Contract

An operator can scan the overview and see what failed, how long it took and when, without decoding enums or seconds. Each view has a clear title, one primary action and secondary actions of lower visual weight. Exact identities, outcomes, durations and timestamps remain one interaction away. All existing routes, authority and confirmation flows behave identically. Light, dark, system and compact modes, a 320px width, keyboard-only use and reduced motion all work.

False completion includes: a restyle that hides exact outcomes or identities; humanized values that change stored data or invent unknown timing; removing or merging routes or confirmations; new remote assets; and checking only empty pages.

## Decisions

### Decision: Token layer in web.css, shared helpers in ui.js

**Choice:** Define a token layer in `web.css`: spacing scale, type scale, radii, elevation, motion durations and semantic status color pairs. Retain existing token names as aliases where other files consume them. Add a small served `ui.js`, loaded before the other scripts, that exposes `window.KilnUi` with pure presentation helpers: status mapping, duration and relative-time formatting, and status-pill element construction. Every view uses these instead of local formatting.

**Rationale:** One layer avoids divergent per-view styles. Pure helpers are deterministic and testable through the existing browser checks. Loading `ui.js` first keeps the deferred script order valid.

### Decision: Human labels with exact values preserved

**Choice:** Status pills show an icon, a human label (Passed, Failed, Running, Queued, Cancelled, Timed out, Denied, Provider error, Unknown, Pending) and carry the exact canonical outcome in their accessible name and title. Durations are humanized (`4m 33s`) with the exact recorded value in `title` and the datetime element. Times are relative (`2 days ago`) with the absolute local and UTC instant inspectable. Missing values render as "Not recorded", never zero.

**Rationale:** Scannability without losing exact evidence or inventing observations.

### Decision: Hierarchy and progressive disclosure

**Choice:** Each page has a header with title, short description and one primary action. Refresh becomes a single quiet icon button per page. Session actions move into an account menu in the top bar. Tables replace card stacks for list data on wide screens and collapse to stacked rows on narrow screens. Exact identities stay in disclosures or copyable monospace fields.

**Rationale:** Modern product convention: clear primary action, lower-weight secondary actions, data-dense but readable lists.

## Risks / Trade-offs

- Label changes may break tests or documentation that assert text. Grep tests and docs before changing strings, and update them in the same change.
- Relative times depend on the browser clock. Exact instants stay visible on hover and focus, and nothing is persisted.
- Parallel restyling of CI, work and review assets needs one shared token and helper contract to avoid divergence.
- Visual verification must use retained real CI data and the live review configuration, not only empty states.

## Verification Strategy

Capture before/after desktop and 320px screenshots on real data for overview, runs, run detail/logs, workers, issues, planning and code review. Exercise light/dark/system, compact density, keyboard navigation, the command palette and reduced motion. Check for document overflow, 44px targets and contrast. Run workspace tests, Clippy, formatting, `node --check` on edited scripts, the full Nix flake check, and Cairn validation and traceability.
