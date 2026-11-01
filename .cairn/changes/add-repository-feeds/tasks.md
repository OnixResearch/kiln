## Phase 1: Core

- [x] [serial] Add the optional feeds profile, dedicated token, durable subscription state and admission bounds. r[kiln.web.feeds.config] r[kiln.web.feeds.bounds]
- [x] [serial] Add URL admission, DNS-pinned bounded HTTPS fetch, redirect re-admission and RSS/Atom parsing. r[kiln.web.feeds.security] r[kiln.web.feeds.bounds]
- [x] [serial] Add Atom rendering for the repository index and per-repository retained runs. r[kiln.web.feeds.publish]

## Phase 2: API and UI

- [x] [serial] Add operator add/refresh/remove and viewer list routes without holding the engine lock across network I/O. r[kiln.web.feeds.subscriptions]
- [x] [serial] Add bounded background refresh for due subscriptions with a configurable cadence. r[kiln.web.feeds.subscriptions.scenario.background] r[kiln.web.feeds.bounds]
- [x] [serial] Export repository feeds as OPML and expose it in the console. r[kiln.web.feeds.publish.scenario.opml]
- [x] [serial] Import OPML subscriptions as bounded pending feeds with operator-only admission. r[kiln.web.feeds.subscriptions.scenario.import]
- [x] [serial] Add a URL-restored, bounded server-side filter to the Feeds page. r[kiln.web.feeds.ui.scenario.search]
- [x] [serial] Publish RSS 2.0 repository and index feeds alongside Atom. r[kiln.web.feeds.publish.scenario.rss]
- [x] [serial] Accept the feed token through a bearer header as an alternative to the capability URL. r[kiln.web.feeds.token]
- [x] [serial] Allow operators to rename subscription labels with bounded validation. r[kiln.web.feeds.subscriptions.scenario.rename]
- [x] [serial] Reload a rotated feed token file without restarting the service. r[kiln.web.feeds.token]
- [x] [serial] Add conditional ETag/Last-Modified refresh with 304 retention and validator bounds. r[kiln.web.feeds.security.scenario.conditional]
- [x] [serial] Add the Feeds console page with publication URLs, subscriptions and bounded external entries. r[kiln.web.feeds.ui]

## Phase 3: Verification

- [x] [serial] Add parser, URL, fetch, rendering, API and browser tests, including private-address and redirect rejection. r[kiln.web.feeds.security] r[kiln.web.feeds.publish] r[kiln.web.feeds.subscriptions]
- [x] [serial] Run focused and workspace tests, Clippy, rustfmt and Cairn gates, then record evidence. r[kiln.web.feeds.bounds]
