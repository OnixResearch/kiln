## Why

The operator asked to improve the CI views and selected four improvements. A real-data audit of a failed run found these problems:

- The error output sits below the fold, under node, stream, cursor and search controls.
- Rerun and Cancel are hidden inside a disclosure.
- Projects have no history view.
- The run list is a narrow sidebar of cards.
- Active runs show no live progress.

## What Changes

- **Failure-first run view:**
  - Failed runs open on the failing node's output, with stderr preferred, error lines highlighted and a jump-to-first-error control.
  - Log controls collapse into a compact toolbar.
  - Rerun and Cancel become visible header actions that use the existing confirmation dialog.
- **Project detail page:** a per-project page built from existing run history. It shows a recent-runs timeline, the current pass/fail streak, a duration trend over runs with recorded timing, the last success and last failure, and Run/Logs actions.
- **Runs table:** a sortable, filterable table of retained runs grouped by day, with run detail opening alongside on wide screens and below on narrow ones.
- **Live runs:**
  - Active runs show a ticking elapsed time, node progress and auto-following logs from the existing live stream.
  - The overview highlights running work.

## Impact

Consumers are Kiln developers and CI operators using the live console, and Kiln web-service maintainers own these views. The durable contribution is faster failure diagnosis and per-project history over existing bounded APIs. The change affects:

- `crates/kiln-web-service/src` web assets;
- new page assets served through the existing static route table;
- browser regression tests;
- operator documentation.

## Out of Scope

- No API, storage, authority, execution or provider changes.
- No new remote assets.
- No fabricated data:
  - runs without timing stay "Not recorded" and are excluded from trends;
  - highlighting is presentation only;
  - the exact retained bytes stay inspectable.
- Controls keep their existing exact-identity confirmations.
