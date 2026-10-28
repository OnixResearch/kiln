## Context

A new-user walkthrough of all twelve live workspace pages found hash-heavy lists, non-actionable overview totals, project controls below a long inventory, an empty initial log tab, opaque-ID-only evidence/comparison entry and mobile menu/focus issues.

## Decisions

### Decision: Retain the existing shell and authority

Repair web.html, web.css and web.js without changing APIs or durable state. Common summaries and choices precede exact identities; exact data remains accessible. Controls continue to require the existing confirmation and server admission. Never execute production mutations during browser verification.

### Decision: Separate frontend ownership

Parent owns web.js shared routing/session, overview/projects, run/log/evidence/comparison UX, lifecycle, integration and verification. Presentation worker owns web.css and web.html only. Browser worker owns only web.js functions loadCobs/renderCobs/loadSource/source renderers/loadReviewsPage and their local helper functions; no shared routing or other files. Communicate markup contracts to parent. All workers skip builds/tests/formatters; parent runs checks after integration.

### Decision: Exercise real read-only paths

Use browser asset interception to serve current frontend source against the real authenticated API, preserving response data. Walk every page and run tab; verify filters, invalid selections, back/reload, keyboard/mobile navigation and light/dark contrast. Read-only actions and dismissing confirmation dialogs do not start jobs. Repository documentation records durable UI behavior and bounded evidence.

## Risks / Trade-offs

No claim of arbitrary backend correctness, CI success, accessibility certification or exhaustive state exploration. Keep explicit missing data and failed reads; never substitute another selected identity. Deployment/restart or publication requires separate explicit authority.

## Verification Strategy

Validate proposal/design/task gates before implementation. Browser-observe initial defects, then exercise integrated real frontend with existing live read-only APIs, including small viewports and keyboard operation. Run applicable frontend syntax and kiln-web-service checks once integrated. Preserve service authority and unrelated files.
