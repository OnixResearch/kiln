# Design: interactive offline dashboard

## Context

The HTML report is deterministic and read-only, but only the CLI-selected run and attempt have detail views. Existing bounds and redaction remain authoritative.

## Goal and Success Contract

- Operators search, filter and sort only the admitted run rows.
- Selecting a run or one of its displayed attempts shows its exact admitted detail and view identity, without re-running the CLI.
- Details are projected only for listed runs, displayed attempts, active attempts and explicit selections. Truncation and redaction remain visible.
- Fixed inline JavaScript manipulates escaped DOM nodes. No dynamic HTML parsing, remote resource, network call, CI command or canonical mutation is added.
- URL fragments retain filters and selected identities across watch reloads; stale identities never select a different target silently.
- The page remains readable without JavaScript, keyboard accessible and usable at narrow widths.
- Same inputs generate identical bytes; watch keeps its existing atomic write and reload cadence.
- The first desktop viewport prioritizes admitted summary and run navigation, with selected details adjacent to the list. Narrow screens stack those surfaces without document-wide horizontal overflow.
- Keyboard users can focus search and inspect shortcut help without shortcuts stealing input from editable controls; explicit theme selection survives fragment reloads.
- Developers generate a representative admitted preview with one documented Cargo example; source changes can be rebuilt through the documented existing toolchain without a server or new frontend dependencies.

False completion includes filtering beyond the admitted bound, showing one run's attempt under another run, hiding missing selection or truncation, introducing a server or external dependency, or retaining the obsolete script-free contract.

## Decisions

### Decision: HTML-only bounded detail projection

**Choice:** Build an HTML report containing the existing base report and additional admitted detail views. Text rendering does not compute these views.

**Rationale:** The observation core remains the only source of facts and identities, and terminal rendering incurs no unnecessary detail projection.

### Decision: Progressive enhancement over pre-rendered details

**Choice:** Render semantic tables, buttons and detail panels, and include one fixed inline script with no serialized journal or raw provider data. Use textContent and DOM attributes for interaction.

**Rationale:** Offline file URLs work without a server; escaping is shared with existing rendering. Native controls provide keyboard interaction.

### Decision: Fragment state and document refresh

**Choice:** Preserve selected IDs and filters in URLSearchParams in the fragment. Watched reports embed the admitted interval for a fixed JavaScript timer calling `location.reload()` and retain meta-refresh inside `noscript` as a fallback.

**Rationale:** Chromium's meta-refresh returned to the original URL and discarded fragment changes during the real watch smoke. `location.reload()` preserves the current URL without browser storage or a new polling transport. No-JavaScript readers still get the original document refresh behavior.

### Decision: An operator console over the same admitted DOM

**Choice:** Keep the fixed script and semantic pre-rendered facts; use a responsive navigation rail, summary cards and master/detail layout. Native disclosures hold secondary provenance and evidence. Canonical view identities remain readable in selected panels. Theme lives in the fragment, not browser storage. Native dialog and controls provide shortcut help.

**Rationale:** Operators need a shorter path from recorded outcome to exact attempt. Secondary evidence remains accessible without dominating the initial viewport. No new projection, provider meaning or runtime service is justified by presentation work.

### Decision: A maintained offline preview example

**Choice:** Add a developer-only Cargo example that produces a canonical journal, admits it through the existing profile/report path and writes an HTML preview with mixed outcomes and retry history. Document one-shot generation and a source rebuild loop using the existing toolchain.

**Rationale:** The current all-pending sample does not exercise realistic visual states. A real admitted fixture and renderer provide a repeatable consumer path without framework, bundler, web server or hand-authored fake report data.

## Risks / Trade-offs

- HTML size grows with the admitted run/attempt bounds. Only bounded displayed lists and explicit selections may contribute details.
- Inline script is an intentional contract change; archived evidence remains historical rather than being rewritten.
- Browsers may ignore local-file refresh instructions. Manual reload still reads the atomically replaced report.
- A restrictive browser policy may disable JavaScript; admitted facts remain readable, but enhanced controls are unavailable.

## Verification Strategy

Run focused projection tests for bounded detail parity and redaction, deterministic rendering and hostile identifiers; exercise real generated files in Chromium for filtering, selection, stale fragment state, keyboard use, reload and narrow layout. Run workspace and repository lifecycle gates and report their actual results without extending claims to live CI or release authority.
