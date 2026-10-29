## Context

The console now has a token layer, shared components, `KilnUi` helpers, an icon rail, a page frame, a dashboard overview and a split run view (`modernize-live-console`, `restructure-live-console-layout`). Secondary pages were only placed into the frame. Accepted requirements still bind:
- `kiln.web.visual_design`, `kiln.web.readable_observations` and `kiln.web.console_layout`;
- `kiln.web.minimal_console` and `kiln.work.usability`;
- the `complete-live-dashboard` requirements;
- the review requirements.

## Goals and Success Contract

On every page, an operator can tell in one glance what the page is for, what needs attention and what the primary action is. Exact identities and non-claims stay reachable without dominating. Pages are consistent with each other.

False completion includes:
- deleting boundary or non-claim text instead of disclosing it;
- hiding exact identifiers entirely;
- inventing summary data;
- changing routes, ids or authority;
- checking only empty pages;
- skipping narrow or dark views.

## Decisions

### Decision: Audit-then-fix per page with retained evidence

**Choice:** Each page is audited in a real browser on real retained data. Findings use a fixed checklist: hierarchy, primary action, copy length, identifiers as primary text, component consistency, states, overflow and truncation, focus and touch, and dark mode. The page is then fixed and re-captured. Evidence records before/after screenshots and a findings list per page.

**Rationale:** Concrete, verifiable improvements instead of a generic restyle.

### Decision: Progressive disclosure for boundary copy

**Choice:** Long boundary and non-claim paragraphs collapse into a labelled "About this view" disclosure or info tooltip near the page header, or into a section footnote. The text is preserved verbatim or kept semantically equivalent. Short, decision-relevant warnings (e.g. confirmation dialogs, unavailable states) stay inline.

**Rationale:** The copy is a contract, but it should not crowd out the data.

### Decision: File ownership for parallel work

**Choice:** CI pages (web.html/web.css/web.js), work pages (work assets) and code review (code-review assets) are fixed independently. Each slice uses only the shared components and reports any shared-component need to the CI slice owner.

## Risks / Trade-offs

- Copy changes can break tests and docs that assert text. Grep before changing, and update in the same change.
- The selector-heavy run view and work keyboard interactions are fragile; preserve ids and data attributes.

## Verification Strategy

Use the real-data candidate asset override at 320/768/1440/1920px, light and dark, plus keyboard checks. Assert zero JS errors, zero document overflow and no sub-44px controls at 320px. Run the full repository gates, then deploy and verify the live surface.
