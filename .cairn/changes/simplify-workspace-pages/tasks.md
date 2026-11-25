## Phase 1: Implementation

- [x] [serial] Audit and simplify first-screen page hierarchy. r[kiln.web.page_focus]
- [x] [serial] Exercise the console register and changed controls in the browser. r[kiln.web.page_focus]

## Page audit

| Pages | Decision |
| --- | --- |
| Guide | Retain the numbered register, quick-start links and keyboard reference; shorten the introduction. |
| Overview | Put failed projects and active work ahead of aggregate health. |
| Runs, CI projects, Project detail | Keep the run inspector, source-bound start and exact run history as separate tasks; preserve the eight run tabs. |
| Workers | Lead with configured targets or the selected target; keep observed inventory in a named disclosure. |
| Issues, Inbox, Planning, Triage, Saved views | Lead with local controls and name the results by task; consolidate shortcut links in a disclosure and avoid repeating saved views on their own page. |
| Code review, Source, Patchgate receipts | Keep each exact revision or receipt next to its relevant controls; avoid merging review authority with evidence. |
| Cairn changes, Fleet | Name the work and blockers concisely, retain separate authority and show an unavailable Fleet snapshot explicitly. |
| Activity, Feeds, Evidence | Remove repeated titles; start with the timeline, feed controls or exact run picker, and retain observation boundaries. |
| Automation, Operations, Raw collaboration history | Keep controls, limits and raw records reachable without leading with duplicated framing; preserve authority boundaries. |
| Settings | Put theme and density before access details, with an intact Guide link. |

Underclass Claude Opus 5.5 independently recommended an action queue, clear
blockers, and progressive disclosure of technical context. Its proposed route
mergers were not adopted: the existing URLs and authority distinctions stay.

Chromium rendered all 23 registered destinations at desktop and 390 px with no
document-level horizontal overflow. All eight selected-run tabs rendered against
fresh fixture identities. Browser checks also exercised the worker inventory
and work shortcuts disclosures and the theme selector. Fleet rendered its
unavailable state; the disposable fixture does not run a Fleet coordinator.
