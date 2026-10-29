## Why

The operator reviewed the deployed `modernize-live-console` result and found it looks the same as before. That pass kept the shell structure: full sidebar, unchanged page order, and overview and run layouts. Its changes were mostly typographic and component-level. The operator chose a structural redesign that keeps the Kiln colours.

## What Changes

- Replace the full-width sidebar with a slim icon rail. The rail expands on hover or focus, can be pinned open, and stays a drawer on mobile. Add a slim top bar with centred search and account, and a centred roomier content column with large page titles.
- Turn the CI overview into a dashboard built only from existing bounded APIs:
  - a health summary with a pass-rate bar over retained latest runs;
  - attention cards for failing projects, with direct Logs and Pipeline actions;
  - a table of recently passed projects;
  - a side column with worker capacity and recent activity.
- Make the run view a split pane: a collapsible run list, then pipeline and logs side by side on wide screens with logs in a terminal-styled panel, and the remaining tabs below.
- Align work and code-review pages to the same page frame and header.

## Impact

Consumers are authenticated Kiln developers and CI operators. Kiln web-service maintainers own the layout. Durable contribution: a reusable page frame (rail, container, page header, dashboard grid, split view) that later views adopt without new local layout code. Affected: `crates/kiln-web-service/src` web assets, the browser regression test, and operator documentation.

## Out of Scope

No API, storage, authority, identity, route or provider changes. No new remote assets. The palette and readable-observation helpers are unchanged. The offline report and hosted bridge are unchanged. No invented data: unavailable worker or activity reads stay explicit.
