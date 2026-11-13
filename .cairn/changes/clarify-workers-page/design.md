## Context

`GET /api/workers` already supplies a bounded `kiln.web-workers.v1` inventory: deterministic target IDs, provider profile and platform, assigned projects, admission-slot counts, slot status and latest retained nonqueued execution. Optional heartbeat observations add `online`/`offline` liveness and observed slots, active/free counts and last-seen time. The default deployment has no heartbeat observations. The server can override a target's status to `offline`; the current four-option UI does not name that state.

The current renderer and inventory validator live in `web.js`. Overview also calls its worker badge and validator, so these callers must migrate together. The console already owns routing, session admission, request cancellation, top-bar read feedback, exact run/project navigation, formatting, clipboard feedback and shared panel/table/disclosure components.

## Goals and Success Contract

A reader can scan targets, distinguish admission availability from heartbeat evidence, find one by project/provider/platform, and read its current work and last execution before exact identifiers. Refresh, URL restoration, history, session clearing and unavailable data remain truthful.

False completion includes claiming that free project slots are online machines; treating a previous passed run as current worker health; showing expired heartbeat free counts as current capacity; silently substituting a missing or filtered target; losing any exact binding/run identity; or retaining private inventory after session clearing.

## Decisions

### Decision: A register first, a readable target detail second

With no selection, show three primary capacity facts (occupied project slots, free project slots, queued runs), a compact configured/enrolled/target count, and the full-width register. Explain once that these are admission slots, not a machine census or dispatch guarantee. Register rows lead with assigned project labels and retain provider/platform metadata secondarily; exact target hashes move out of the primary text. Rows list slot state, heartbeat evidence, queue and latest execution, and sort deterministically by readable target label with binding/ID tie-breakers.

A selection opens a full-width detail sheet instead of an empty-inspector master/detail layout. Its order is: readable target title; slot/heartbeat state; capacity and latest execution; assigned projects with enrollment, queue and exact active-run links; exact target/provider/platform/snapshot facts behind a disclosure. All targets clears only the selection and restores focus to that target's register link. The register remains available under Browse targets, with its filters intact. A selection hidden by filters is stated and never changed. A missing target is shown as missing with the requested identity and a return path.

### Decision: Keep the existing shell and introduce one page module

Serve `/workers.js` and `/workers.css`. Load the script after `ui.js` and before `web.js`, because the shell's Overview and route parser use its frozen worker-state vocabulary and badge/validation functions. Export `load`, `clear`, the shared badge/validator/vocabulary and the pure presentation helpers. Remove the old page renderer, obsolete shell listeners and styling. Do not leave compatibility renderers or aliases.

`load(container, ctx)` receives the existing `api`, `navigate`, route and an `isCurrent` predicate. A generation guard plus the shell's current-route/session predicate rejects delayed responses. `clear()` invalidates reads/timers, removes private DOM and drops the retained inventory. Refresh still uses the existing shell action and re-reads the endpoint. An error replaces the capacity view; storage failure never renders free capacity.

### Decision: Immediate bounded filtering without refetching per keystroke

Filter the loaded inventory locally. Use the existing 256-character query bound and 350 ms URL-commit pause. Commit `worker_q` and `worker_status` without replacing the selected target. Handle a self-applied filter URL update without another API read or DOM redraw; browser-history changes still restore fields and selection. Keep focus/caret in the search field while typing and a polite count on the result set. Clear filters does not clear selection. Arrow keys move among visible target links; opening a detail focuses its heading, and returning focuses its original row.

### Decision: Present existing facts, not health policy

The vocabulary names `slots_available`, `busy`, `unknown`, `not_enrolled` and the API's `offline` state. Slot availability is neutral rather than a health success. Unknown/unrecognized states remain explicit. Overview uses the same vocabulary rather than retaining a second rendering convention.

Heartbeat states read as a recent report, a noncurrent report or not observed, preserving the recorded liveness value in the exact-binding disclosure. The API can return `offline` with a zero aggregate and no last-seen timestamp when monitoring is configured but no matching heartbeat is retained. That is not an expired report. Keep admission counts separate from heartbeat counts, and describe API-projected counts as snapshot facts rather than inventing the last producer report. A noncurrent payload's reported free slots are never presented as currently usable capacity. The optional last-seen timestamp stays relative with its exact instant available through `KilnUi`.

Latest execution uses the shared outcome, time and duration components, labels itself as a retained execution rather than worker health, and links its exact run. Queued requests never stand in for execution evidence. Missing times/durations/counts stay not recorded, never zero or now. Assigned project labels link to the existing CI project view; no action dispatches work.

### Decision: Use the shared technical graphics components

Use the identity palette, square framed panels, ruled tables, three-cell readouts, monospace technical labels and existing disclosures. Reuse `data-table-stack` so register rows stack by their own region width. Controls stay 44px at narrow widths; long names/IDs wrap without document overflow. No bespoke colour, radius, blurred shadow, icon or remote asset. Add `workers.css` to the existing identity rail.

## Verification Strategy

Permanent Node tests cover consumer-visible boundaries in pure helpers: deterministic nonmutating target ordering, filtering without substituting selection, valid zero versus missing counts/times, latest-execution timing, and online/offline/absent heartbeat presentation. Test positive and negative cases together. Do not pin prose or test script wiring.

Run the existing Node rail and JS syntax checks once after implementation. Run `cargo test --locked -p kiln-web-service`, crate rustfmt and Clippy for embedded-asset route changes using the available pinned toolchain; record any dev-shell/flake rail not run rather than substitute its claim.

Browser review uses real assets in a throwaway loopback preview, read-only live inventory and explicit synthetic edge scenarios. Cover the list and selected detail in light/dark at 1440, 768, 390 and 320px; counts and filters; exact selection/history restoration; keyboard return/focus; actual run links; absent/unknown/expired heartbeats; unknown execution; unenrolled/empty/missing/error/storage-failed cases; and a delayed read followed by session clearing. Do not change the installed service or invoke a worker operation. Record console/network observations, overflow and touch-target measurements with their bounds.

Run proposal/design/tasks gates before implementation, then Cairn validation, policy freshness and traceability with this change's delta root selected. Operator docs and the changelog describe the new presentation and non-claims. The maintainer owns the reusable page; operator approval still owns activation and release/lifecycle transitions.

## Risks and Trade-offs

A detail replaces the full register rather than keeping a narrow card rail; All targets, keyboard focus restoration, the URL and Browse targets retain orientation. The browser preview does not qualify the deployed service or worker behavior. Heartbeat/backend schema drift remains bounded by the unchanged inventory contract and explicit unavailable/unknown presentation.
