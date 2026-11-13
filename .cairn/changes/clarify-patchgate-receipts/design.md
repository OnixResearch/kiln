## Context

A `kiln.patchgate.review.v1` receipt holds three parts. `request_identity` is the exact request. `observation` is Kiln's admitted observation: content identities, the `isolated` and `authorizes_acceptance` claims, and one normalized decision per change. `native_report` is Patchgate's own JSON report, kept as a string so that every byte survives. In that report, `items` record each change's message, paths, state, prerequisites (`needs`) and whether it was decided in this pass. `records` record each decision's `decision`, `by` (`gate`, `model` or `human`), `reason`, `summary`, `at`, and model, coverage, provenance and verification evidence. The report also records the main and inbox channel states, the trusted configuration and `review_usage`.

Kiln admits nine normalized decisions (`kiln_core::review::ReviewDecision`). They are Patchgate's own item states (`patchgate/src/gate.rs`, `State`):

- `pending`: no decision yet.
- `waiting`: a prerequisite is not in main.
- `blocked`: a prerequisite is "stuck", meaning rejected, escalated, removed, blocked or unknown.
- `accepted`.
- `removed`: accepted earlier, no longer in main.
- `rejected`.
- `escalated`: a person decides.
- `reverted`: removed by `patchgate revert`.
- `unknown`: a decision record Patchgate does not recognize.

`patchgate explain` gives a next step for each state. The README says what each decider does.

The console renders receipts in two places. `loadReviewsPage` in `web.js` draws the page over `/api/reviews`. `renderPatchgateReceipt` draws a run's Reviews tab over `/api/runs/{id}/reports`. The two differ, and both are described under Why in the proposal. Run-bound records carry `recorded_at_ms`, `node_id`, `source_revision`, an `artifact` with the exact bytes' size, BLAKE3 and download path, and `unavailable` when a declared report is missing.

Accepted requirements bind the work. `kiln.dashboard.reviews.view` is modified here. `kiln.patchgate.nonclaims` (no acceptance authority) and `kiln.patchgate.worker.job` (a missing report stays unavailable) hold unchanged. So do `kiln.web.page_consistency`, `kiln.web.readable_observations`, `kiln.web.design_system`, `kiln.identity.graphics_standard` and `kiln.web.wayfinding`.

## Goals and Success Contract

A reader finds a receipt by what it reviewed and when. They then see, for each change, what Patchgate decided, what that means, who decided, why, when and what happens next. Every exact identity and the verbatim report stay one step away.

False completion includes:

- any of the nine admitted decisions reading as unrecognized or taking another decision's tone;
- a native fact shown as if Kiln had verified it;
- a native state shown in place of the normalized decision;
- a receipt whose native report cannot be read losing its decisions;
- a filter that replaces the selected receipt;
- the page and the run tab drawing a receipt differently;
- a declared-but-missing review report reading as clean;
- a new route, mutation or authority claim.

## Decisions

### Decision: One page module for the page and the run tab

**Choice:** Add `receipts.js`, served at `/receipts.js` and loaded after `web.js` like `project.js` and `source.js`, with `receipts.css` at `/receipts.css`. The module exports `load(container, ctx)` and `clear()` for the page, `renderRun(parent, result, ctx)` for the run tab, and pure helpers for tests. `web.js` keeps routing, the session clear and the artifact download. It passes `api`, `navigate`, `route` and `download` in `ctx`, and drops its receipt functions (`retainedDecisionLabel`, `reviewDecisionBadges`, `loadReviewsPage`, `patchgateReceipts`, `decisionPill`, `renderChangeRef`, `renderPatchgateReceipt`, `renderReviews`).

**Rationale:** Project history and Source already render through their own modules. One receipt sheet makes the run tab and the page agree by construction, and `web.js` shrinks.

### Decision: A decision vocabulary grounded in Patchgate

**Choice:** One frozen table maps each of the nine decisions to a label, a semantic tone, a meaning and Patchgate's next step:

| Decision | Tone | Next step (from `patchgate explain`) |
| --- | --- | --- |
| accepted | success | none |
| rejected | failure | Remove or replace the change in the inbox, then record a corrected change |
| escalated | warning | Resolve it in a separate workspace, or reject it |
| blocked | warning | Decide the prerequisites first |
| waiting | queued | Decide the prerequisites first |
| pending | queued | Review the change |
| removed | cancelled | Examine the removal from main before deciding again |
| reverted | cancelled | Examine the removal from main before deciding again |
| unknown | unknown | Examine the channel and its prerequisites |

Any other string reads as "Unrecognized: <value>". A missing value reads as "Not recorded". Both take the unknown tone. The tally orders decisions by attention: rejected, escalated, blocked, unknown, unrecognized, missing, removed, reverted, waiting, pending, accepted. "Needs attention" is Patchgate's stuck set (`State::stuck`: rejected, escalated, removed, blocked, unknown) plus unrecognized and missing values.

**Rationale:** The table follows Patchgate's own model, so a meaning or next step is never invented. Blocked takes the warning tone, because it needs an action and is not a rejection of this change. Red stays reserved for rejection.

### Decision: Recorded context is labeled, bounded and never replaces a decision

**Choice:** A pure reader, `nativeContext(document)`, parses `native_report` once inside a try block. It reads only own properties with type checks, keys changes by hash in a `Map` and ignores unknown fields. It returns three things: the report-level context, a per-change context (message, paths, item state, `decided`, reason, note, prerequisites with their states, and the matching record's decision, decider, user, model, time, summary, execution error, reuse, Nix check names, context coverage and model request count), and a status (`read`, `unreadable` or `absent`). The sheet always takes each change's decision from `observation.reviews`. Native facts sit in that change's entry under a "Recorded by Patchgate" label. When the report is unreadable, each entry shows the normalized decision and states that the recorded context is unavailable. Text is written only through `textContent`.

**Rationale:** The normalized observation is what Kiln admitted. The native report is producer evidence that a reader needs in order to understand a decision, but it must not stand in for that decision.

### Decision: The inventory carries two bounded list facts

**Choice:** `reviews.rs` adds two fields to each inventory summary:

- `first_change_message`: the first line of the native item whose hash equals the first normalized change. Control characters become spaces, and the line is cut to 160 characters ending in "…". It is null if there is no such item or message.
- `latest_decision_at_ms`: the latest record `at` that `jiff::Timestamp` parses as RFC 3339, in milliseconds. Unparseable times are ignored, and it is null if none remain.

A receipt whose native report is not a JSON object yields null for both. The server keeps its name order. The page sorts the list newest first, with untimed receipts last by name.

**Rationale:** The list is bounded at 256 receipts, and naming each by its change would otherwise take one detail request per receipt. The same authenticated readers can already read each detail. The parse is bounded by the existing 1 MiB receipt limit. `jiff` is already a pinned dependency.

**Rejected:** a detail request per row, which costs N requests; the full native report in the inventory, which could pass the 4 MiB response bound; and parsing native reports for the list in the browser, which needs the same N requests.

### Decision: Client-side filtering, restored from the URL

**Choice:** The inventory is already bounded and complete, so the text and decision filters narrow the list as the reader types. They commit `receipt_q` and `receipt_decision` to the URL after the Source page's 350 ms pause. A selected receipt that the filters hide stays selected, and the list says so. ArrowUp and ArrowDown move focus between receipt links.

### Decision: Information order, from the human facts to the exact ones

**Choice:** The receipt sheet runs in this order:

1. Headline and tally.
2. The evidence-only statement, with warnings for a producer claim of acceptance authority, isolation not recorded, or an unreadable report.
3. Changes.
4. Review context.
5. Exact identities.
6. The verbatim Patchgate report, closed.
7. The complete receipt JSON, closed.

The identities use the shared fact table. A change hash has a copy control. `copyText` moves from `source.js` into `ui.js` for both pages.

**Rationale:** `kiln.web.page_consistency` puts readable primary text first and exact identifiers second, reachable but not leading.

### Decision: Run-tab specifics

**Choice:** Each run-bound receipt adds its binding (recorded at, node, source revision, report record) and a "Download exact receipt" button. The button uses the shell's existing artifact download over `record.artifact`. A `review` record with `unavailable` is shown as "Review report unavailable (missing|unreadable)", and no decision is inferred. Other report kinds stay under Tests & coverage, with a count.

## GPUI Kit Design Guides and graphics standard

The sheet uses the shared panel, fact table, status pill, disclosure and empty-state components, ink frames and the monospace label layer. It adds no radius, no blurred shadow and no new colour. Identity red is not used. `receipts.css` joins the identity check's console surface list.

## Risks / Trade-offs

- **Native schema drift.** Patchgate may rename or add native fields. The reader ignores unknown fields, and a missing field reads as not recorded. The verbatim report and the complete receipt always remain.
- **Inventory exposure.** The inventory now carries change messages. The same viewer and operator principals could already read them in each receipt's detail. Workers stay denied.
- **Two readers of one field.** The server's list message and the browser's per-change message both read `items[].message`. Tests on each side pin the shared rules: first line, the first normalized change, and the 160-character bound applying only to the list.
- **Synthetic preview data.** The browser review uses the real Rust reviews API over synthetic receipts plus one real receipt layout. It is not the deployed service.

## Verification Strategy

Node (`tests/live-console-ui.test.mjs`) covers:

- the decision vocabulary: all nine states, unrecognized, missing, tones, next steps and the attention set;
- the tally order and counts;
- `nativeContext` over a full report, a partial report, a non-JSON string, a non-string, hostile keys (`__proto__`, `constructor`), wrong types and a mismatched hash;
- list rows: sorting newest first with untimed rows last, search over name, message, hash and identities, and the decision and attention filters;
- the decider label for gate, model, human, reused and unknown deciders.

Rust (`tests/reviews.rs`) covers `first_change_message` and `latest_decision_at_ms` for a full report, a report without items, a non-JSON report, a message belonging to another change, invalid and mixed-offset times, control characters and the 160-character bound.

Browser: the real assets over the real Rust reviews API, at 1440px and 390px, light and dark. The check covers every decision, the anomaly receipt, the unreadable-report receipt, the filters and their URL restoration, a selection hidden by a filter, the run tab with a receipt and with an unavailable report, console errors, off-origin requests, overflow and 44px targets.

Traceability: the change's delta root is selected in `cairn-policy/development-coverage.ncl` while it is in flight.
