## Phase 1: Core

- [x] [serial] Add the optional feeds profile, dedicated token, durable subscription state and admission bounds. r[kiln.web.feeds.config] r[kiln.web.feeds.bounds]
- [x] [serial] Add URL admission, DNS-pinned bounded HTTPS fetch, redirect re-admission and RSS/Atom parsing. r[kiln.web.feeds.security] r[kiln.web.feeds.bounds]
- [x] [serial] Add Atom rendering for the repository index and per-repository retained runs. r[kiln.web.feeds.publish]

## Phase 2: API and UI

- [x] [serial] Add operator add/refresh/remove and viewer list routes without holding the engine lock across network I/O. r[kiln.web.feeds.subscriptions]
- [x] [serial] Add bounded background refresh for due subscriptions with a configurable cadence. r[kiln.web.feeds.subscriptions.scenario.background] r[kiln.web.feeds.bounds]
- [x] [serial] Add conditional ETag/Last-Modified refresh with 304 retention and validator bounds. r[kiln.web.feeds.security.scenario.conditional]
- [x] [serial] Add the Feeds console page with publication URLs, subscriptions and bounded external entries. r[kiln.web.feeds.ui]

## Phase 3: Verification

- [x] [serial] Add parser, URL, fetch, rendering, API and browser tests, including private-address and redirect rejection. r[kiln.web.feeds.security] r[kiln.web.feeds.publish] r[kiln.web.feeds.subscriptions]
- [x] [serial] Run focused and workspace tests, Clippy, rustfmt and Cairn gates, then record evidence. r[kiln.web.feeds.bounds]
