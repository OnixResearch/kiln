# Fleet console snapshot visualization

## Context

The console consumes a bounded projection, not inventory telemetry or history. Ranked rows are not the ready-and-unclaimed total.

## Goals / Non-Goals

Make composition, currentness and operator inputs legible. Do not infer capacity, progress percentages, trends or command authorization.

## Decisions

Use labeled native bars to show the share of listed attempts in each coordinator state, with exact counts. Distinguish connecting, live, stale and unavailable from the timestamp of the last accepted snapshot. Keep transport and command feedback independent. Render changed registers independently and preserve decision drafts by exact identity. Use shared theme tokens and stack rows and form fields at narrow widths.

Place both interactive breakdowns above the attempts register. Each bar is a keyboard-operable toggle with a visible count. The charts retain snapshot-wide denominators while the register shows the intersection of selected facets. Keep selected facets across snapshots, including when they no longer match, and provide a single clear action. A missing session machine is a distinct category, not an inventory assertion.

Poll the canonical Cairn change index independently while Fleet is visible. Show the observed change lane and distribution of unit lanes, not a completion percentage. Keep last accepted rows on a failed refresh and mark them stale. Use repository identity and change name for exact navigation.

Project bounded current active attempts from the Cairn index, including worker, provider, harness, requested model artifact and admitted model-service references from the exact retained dispatch plan. Join Fleet-managed sessions on attempt identity and fence; a session can name a machine and agent kind. Label dispatch values as planned, and do not infer a serving model, physical Lattice host or subagent topology without provider observations.

## Risks / Trade-offs

Snapshot bounds can exclude attempts, so the chart names its denominator. The last accepted snapshot remains readable during an outage but is explicitly stale. Mutations still use the existing endpoints and recorded identities and fences.
