# Add an HTML report to the operator dashboard

## Why

The dashboard renders a terminal report. That is the right surface for an
operator at a shell, but it is the wrong surface for review, for sharing, and
for a browser: the report cannot be archived as a page, linked, or read on a
machine without a terminal.

The dashboard already holds everything an HTML report needs. It admits the
journal or the store, projects the accepted views, and renders the facts. Only
the last step is terminal-shaped, so the HTML surface is a renderer over the
same admitted report rather than a second projection.

## What Changes

- Add a self-contained HTML renderer to the dashboard crate: inline style, no
  script, no external resource, no clock, and no environment input.
- Add `--format text|html` and `--output <path>` to the dashboard shell, so
  the report can be written to a file as well as to standard output.
- Add a browser property rail that drives the rendered document in Chromium
  and states what the document must show and must not contain.
- Add the tests that bind the HTML report to the terminal report.

## Impact

- **Affected specs:** a new `dashboard-html-surface` capability.
- **Planned files:** the dashboard crate's HTML renderer, the shell's format
  and output options, the browser specification, the tests, the operator
  documentation, and the lifecycle package.
- **Compatibility:** additive only. The terminal report is unchanged and stays
  the default format. No admitted view, bound, or redaction rule changes.
- **Testing:** the dashboard crate's tests, the browser rail, the workspace
  gates, and the Cairn gates.

## Out of Scope

- No second projection and no new view vocabulary. The HTML report renders
  the same admitted views the terminal report renders.
- No interactivity, no script, and no live refresh in the browser. The
  document is a snapshot of one admitted revision.
- No server, no route, and no network. The dashboard stays a local file
  reader; the HTML surface is a file it writes.
- No claim that the browser rail explored every document. It states properties
  over the document it drives.
