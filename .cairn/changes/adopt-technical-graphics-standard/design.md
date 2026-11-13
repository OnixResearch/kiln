## Context

`establish-kiln-visual-identity` gave every Kiln browser surface one identity stylesheet, `crates/kiln-dashboard/src/render/identity.css`, included first by the live console (`/identity.css`), the offline catalog and report (inline) and the hosted bridge page (joined into `/app.css`). Surface stylesheets carry no colour of their own; `tests/kiln-identity.test.mjs` holds the contrast floor and token ownership.

That identity is warm and decorative: porcelain and soot neutrals, an ember gradient on primary actions and the current navigation tile, a glowing arch mark, a graduated tick rule, radii from .3rem to .7rem and blurred shadows. The operator asked for the NASA graphics standard in the manner of the U.S. Graphics Company, and for every page to make sense to a new and to an advanced user.

The console's wayfinding is spread out. A destination's title lives in a map in `restoreRoute`, its purpose in a second map, its group in a third, and its rail label in `web.html`. The command palette lists eight destinations by hand. There is no page that explains run states or terms. Accepted requirements bind the work: `kiln.web.visual_design`, `kiln.web.readable_observations`, `kiln.web.console_layout`, `kiln.web.page_consistency`, `kiln.web.design_system`, `kiln.web.live_activity`, `kiln.web.request_feedback`, `kiln.web.runs_table` and the `visual-identity` capability.

## Goals and Success Contract

Every surface reads as one technical document. A new operator can find any destination, learn what each run state means and see a first step. An advanced operator can jump to any destination by number from the keyboard. Nothing an operator can do today stops working.

False completion includes: identity red on a status, action or selection; a rounded panel or a blurred shadow left in any surface stylesheet; a destination whose number or title differs between the rail, the title block, the palette and the Guide; a Guide entry for a page or key the console does not have; a running or queued run whose timing reads "Not recorded"; a pressed state lost with the old palette; a changed route, label of record, confirmation or keyboard path.

## Decisions

### Decision: The standard lives in the identity stylesheet

**Choice:** `identity.css` keeps the GPUI Kit–aligned token names and changes their values:

- Paper and ink neutrals: page `#ececec`, panels `#ffffff`, ink `#0b0b0b`; dark appearance inverts to `#0a0a0a`, `#101010` and `#f0f0f0`. A new `--frame` token is the ink line around panels, cards, controls and overlays.
- `--primary` is ink (paper in dark appearance), so the single primary action of a view is a solid ink block. `--ring` is ink.
- Identity red `#fc3d21` (`--brand`, NASA red, PMS 179) is drawn in three places only: the brand mark, the index block of the title rule and the request-progress line. The mark's door is ink (`--brand-door`) and amber (`--brand-live`) while the live snapshot reports running work.
- Status colours become signal lights with the same meanings: green, crimson, NASA blue, amber, greys and a dashed olive for unknown.
- Type: a Helvetica-class grotesque stack and a monospace stack, both local only. `--label-font` and `--label-tracking` define the technical label: small, upper case, tracked monospace.
- Every radius tier is zero. `--shadow-sm` is none; `--shadow-md` and `--shadow-overlay` are unblurred offsets.
- The title rule is a 2px ink rule opened by a 6px block of identity red.

The ember tokens (`--ember-bright`, `--ember-deep`, `--flame`, `--soot`, `--glaze`, `--shade`, `--primary-fill`, `--primary-fill-hover`) are removed with their uses; no alias remains.

**Rationale:** The accepted identity already routes every palette, type and mark decision through one file, so the standard is a change of values and a few tokens, not a new layer. Ink for action keeps emphasis apart from every status hue by lightness and fill rather than by hue. Reserving red for identity is the manual's rule and keeps it from being read as failure.

**Rejected:** NASA blue for actions (it is the running status colour); red for actions (collides with failure); keeping small radii "for softness" (the standard is square); NASA's logotype (NASA's mark, not Kiln's).

### Decision: The standard is a deterministic check

**Choice:** `tests/kiln-identity.test.mjs` gains `standardFailures`. For the identity and every surface stylesheet it fails on `var(--brand…)` in a rule other than the mark, the title rule or the top-bar progress line; on a literal non-zero radius outside the mark's arch; on a non-zero radius tier; and on any blurred `box-shadow` or shadow tier. The contrast check gains the frame and selected-row boundaries, identity red against the rail and the page, the door against red, the lit door against the idle door, and a distance floor between identity red and the failure colour.

**Rationale:** "Square, flat, red only for identity" is checkable by syntax. A check holds it where review by eye drifts. Shadows are parsed per comma-separated layer with functions removed, so a ring (`0 0 0 2px …`) and a hard offset (`4px 4px 0 0 …`) pass and `0 2px 6px …` fails.

**Bound:** the check reads stylesheets, not computed styles: a colour set from script or inline style is outside it. The console sets none.

### Decision: One numbered destination register

**Choice:** `ui.js` exports `destinations` and `destinationGroups`. Each destination has a page, a number (`0.0` Guide; `1.1`–`1.4` CI; `2.1`–`2.6` work; `3.1`–`3.5` inspect; `4.1`–`4.4` administration), a group, a title, a purpose and an optional parent; the project history page shares `1.3` with CI projects. `web.js` admits exactly the register's pages, writes the title block, breadcrumb and document title from it and adds each rail link's number (hidden from the accessible name). The command palette builds one "Go to N.N Title" command per top-level destination, so typing a number finds it. The rail's CI group moves first to match the numbering.

**Rationale:** The three maps and the rail could disagree; one table cannot. Section numbers are the manual's wayfinding device, give an advanced user a two-keystroke jump, and give a new user a stable way to refer to a page.

### Decision: A Guide page explains the console from the register

**Choice:** `?page=guide` renders four sheets: a first path (Overview, Runs, CI projects, Code review), every destination by number with its purpose, how each run state reads (drawn with the real status pills), the words Kiln uses, and the keyboard reference. Code review's shortcuts come from `KilnCodeReview.keys` and are listed only when that module has loaded. The rail footer and Settings link to it; an empty Overview offers it beside CI projects.

**Rationale:** The Guide reads the same register, status rule and shortcut table the console uses, so it cannot describe a page or key that does not exist.

### Decision: Timing a run cannot have yet is named

**Choice:** `KilnUi.runTiming(run)` classifies duration, start and completion as recorded, in progress, not started, not finished or not recorded from the run's recorded phase through `outcomeStatus`. `runDurationElement` and `runInstantElement` draw the run list, the run header, the Overview project list and a project's run history. A recorded value always wins. Finished, unknown and unreadable runs keep "Not recorded".

**Rationale:** "Not recorded" on a run that has not finished reads as lost data. The words come from the recorded phase only; no elapsed time is computed and nothing is invented, which keeps `kiln.web.readable_observations`' rule against substituted values.

### Decision: Readable quantities

**Choice:** `humanBytes` moves from `source.js` to `ui.js`. Operations shows a byte resource as `1.2 GiB (1,288,490,188 bytes)` with its share of the limit, names the resource in words, and leads a queue entry's run link with its short identity. Worker slot badges take signal tones. A work status outside the built-in set renders as a neutral custom cell, not an unknown state.

### Decision: Pressed and selected states in the flat grammar

**Choice:** A pressed secondary button or row action inverts to ink; a pressed primary takes an inset paper line; a pressed danger button fills with the danger colour; a pressed quiet control takes the selected fill. Selection, the current destination and the active tab are inverted ink cells, with no edge bar.

**Rationale:** The old pressed states used `--shade`, which the standard removes. Inversion is the manual's selection device and needs no colour.

## GPUI Kit Design Guides

The recorded web deviations stand (destinations are links, no OS chrome, 44px mobile targets, the page-header command stays primary). Two changes to conformance are recorded:

- Monospace now also sets technical labels (eyebrows, column heads, field labels, fact terms), not only identifiers, shortcuts and aligned numbers. Reading text, titles and controls stay in the grotesque.
- The radius scale is a single zero tier. Nesting stays concentric because every corner is square.

Selection stays on the item's own surface, one emphasis colour (ink) marks actions, elevation is for stacking only, and motion is for real change.

## Risks / Trade-offs

- `catalog.css` styles both the catalog and the console shell; a change there reaches both. The browser review covers both.
- Local font stacks render differently per platform: Helvetica Neue on macOS, Nimbus Sans or Liberation Sans on Linux, Arial on Windows. Metrics differ slightly; no layout depends on exact glyph widths.
- Ink action emphasis and ink text share a colour. Actions are distinguished by their framed or filled cell, not by hue; the contrast check holds the frame at 3:1.
- Square geometry makes adjacent framed controls read as one block when they touch; clusters keep a gap.
- The synthetic data of the browser review is shaped from the service's serializers; it is not the service. Deployed behaviour is not claimed.

## Verification Strategy

Pure checks in `node --test`: the contrast floor with the new pairs and the red/failure distance, with a lowered colour and a collided failure colour reported; ownership; the standard check over every real stylesheet, with failing fixtures for red on an action and a status, a rounded corner, a rounded tier and a blurred shadow, and an allowed fixture for the mark, rings and offsets; the destination register's invariants; run timing for running, queued, awaiting-approval, recorded, finished, unknown, malformed and non-object runs; binary byte units.

Rust: `cargo test --locked` for `kiln-dashboard`, `kiln-web-service`, `kiln-hosted` and `onix-kiln-cli`, which embed the stylesheets and scripts (determinism, escaping and rendered-size bounds of the offline documents; asset routes). No Rust source changes, so `cargo fmt` and Clippy are not rerun.

Browser, real assets over synthetic data: every console page at 1440px in light and dark, the explicit theme override in both directions, focus ring, the command palette's number search, the empty Overview, signed-out sign-in, the hosted page in both appearances, the catalog and report from the working tree, and 390px and 768px for overflow and 44px targets.

Traceability: the change's delta-spec roots are selected in `cairn-policy/development-coverage.ncl` while it is in flight, and coverage runs under `kiln-active-development`.
