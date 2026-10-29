## Context

`modernize-live-console` introduced a token layer, shared components and `KilnUi` readable-observation helpers, but kept the original shell. The deployed result was judged visually unchanged. The accepted `kiln.dashboard.complete.shell`, `kiln.dashboard.complete.overview`, `kiln.web.minimal_console` and `kiln.web.visual_design` requirements still bind: navigation vocabulary, appearance/density, accessibility, explicit unavailable states, exact Logs links on the overview, 44px mobile targets and 320px containment.

## Goals and Success Contract

A returning operator immediately sees a different product layout: a compact icon rail, a centred content column with a large title, a dashboard overview that leads with failing projects, and a split run view showing pipeline and logs together. Every existing route, navigation label (available to assistive technology and on rail expansion), control, confirmation and keyboard behaviour remains.

False completion includes:
- a rail that hides labels from assistive technology or keyboard users;
- overview sections built from invented or sample data;
- dropping Logs links that open the exact execution;
- a split view that overflows at 320px;
- verifying only empty pages.

## Decisions

### Decision: Rail navigation with labelled expansion

**Choice:** The sidebar becomes a 4rem icon rail. Labels stay in the DOM and visible to assistive technology, and appear on hover or focus-within as an overlay expansion. A pin toggle keeps it expanded; the pin state lives in page memory and the URL is unchanged. Below 60rem it becomes the existing drawer. Group disclosures (More work, Inspect, Administration) become flyout sections in expanded state.

**Rationale:** More content width and a modern look while keeping keyboard and assistive navigation intact.

### Decision: Dashboard overview from existing APIs only

**Choice:** The overview composes `/api/overview`, `/api/workers` and `/api/activity` (bounded and read-only).
- Health summary: failing, passing and active counts plus a pass-rate bar over retained latest runs. The denominator is shown, and unknown runs are excluded from the rate and listed.
- Attention: one card per failing project, with status pill, duration, relative time, and Logs (stderr) and Pipeline links keeping the existing aria labels.
- Recently passed: a compact table.
- Side column: worker slots and status, and the latest five activity entries.

Each section has its own loading, empty and unavailable state.

**Rationale:** Leading with failures matches the minimal-console contract and avoids fabricated metrics.

### Decision: Split run view

**Choice:** On ≥90rem the run page shows a collapsible run list, then the run header, then pipeline and logs side by side, with the other tabs below. Narrower widths keep the tabbed single column. The logs panel uses a terminal style (dark surface in both themes, monospace, line wrapping toggle unchanged). All existing log ids and behaviours are kept.

**Rationale:** Operators inspect pipeline and output together; a terminal panel reads as a log.

## Risks / Trade-offs

- Rail hover expansion must not trap focus or obscure content permanently; Escape collapses it.
- Dashboard sections issue additional bounded reads, which must not change authority or block the page if one fails.
- Split view increases layout complexity; it degrades to tabs below 90rem.

## Verification Strategy

Use the same real-data candidate asset override as before, at 1920, 1440 and 320px, in light/dark/compact and reduced motion. Check rail keyboard expansion and Escape, the overview's Logs link opening the exact run, split-view logs reading, zero overflow and zero JS errors. Then run repository gates (including `node --test`), deploy with the established procedure, and confirm live assets and a real browser session.
