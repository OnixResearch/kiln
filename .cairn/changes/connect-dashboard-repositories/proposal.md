# Connect the dashboard to real repositories

## Why

Kiln operators can execute real repositories, but the dashboard preview is not an operational repository/history surface. Radicle emits candidate-local journals; GitHub retains canonical events in its own replay schema, not the canonical SQLite event-store schema.

## What Changes

- Export an exact GitHub run's persisted events as a replay-checked canonical observation journal using read-only database access and no credential resolution or provider call.
- Admit a bounded, typed Nickel repository/history catalog and render independently admitted observations in one offline HTML document with repository/history navigation and preserved watch selection.
- Generate real reports and deliver only the HTML to Aspen3 through an explicitly configured atomic file-transfer shell, with bounded execution and no credentials in the report.
- Dogfood Kiln's existing source/execution route, exercise success, failure, duplicate delivery and restart recovery where available, and distinguish retained evidence from fresh execution.

## Impact

- Affected capabilities: dashboard repository catalog, GitHub observation export, dashboard report delivery.
- Current consumer: Kiln operators viewing real repository CI on Aspen3.
- Maintenance owner: Kiln dashboard and provider-adapter maintainers.
- Adoption: existing journal/store commands remain supported; operators export reviewed Nickel catalogs and delivery configuration, then run the local publisher.
- Repeatability: positive/negative admission and replay checks, real CLI/browser sessions, and an actual atomic transfer to Aspen3.

## Authority Boundary

No canonical Radicle initialization, signing, push, synchronization, announcements, delegate/threshold changes, GitHub webhook or status writes, or production service mutation is authorized. Existing records and source acquisition are read-only; any unavailable live prerequisite is recorded precisely rather than replaced with synthetic success.
