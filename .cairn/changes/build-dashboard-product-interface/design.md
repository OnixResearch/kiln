# Design: one dashboard application shell

## Context

The catalog already admits every journal independently and embeds complete reports in opaque-origin allow-scripts sandboxes. The actual Rats/Ratzilla widget supplies repository/history selection. These mechanisms remain; the product layer must not invent combined CI identities or reinterpret observations.

## Goal and Success Criteria

One polished, responsive application offers Overview, Projects, Runs, Activity, Evidence and Settings. All navigation/search/preferences work on admitted inputs without a network. Canonical view facts, identity and non-claims remain available. Back/reload preserves route and per-history state. Missing identities never silently select replacements.

## Decisions

- Keep core admission and projections unchanged. Catalog shell owns app routes, project/history metadata search and appearance; child report owns run/attempt facts and filters.
- Render overview facts only as catalog inventory counts or explicitly selected-history observations; never sum repeated snapshots into global run counts or infer freshness.
- Preserve Rats as the primary interactive repository/history index on Projects; retain accessible HTML controls and no-JavaScript reports.
- Use fixed inline CSS/JS and textContent/escaped markup. Add inert app route/theme/density messages to the existing source-checked parent/child bridge. Never grant allow-same-origin or parent DOM access.
- Child route presentation suppresses duplicate standalone chrome only inside the catalog. Standalone reports keep their existing surface. Runs, Activity and Evidence show their own bounded sections; Overview presents selected-history summary.
- Persist global route, theme, density and metadata query alongside exact project/history and per-history fragments in the URL; support browser Back and reload without local storage or credentials.
- Keep delivery a finite separate Aspen3 preview. Existing live index and publisher remain unchanged.

## Risks / Trade-offs

An iframe remains the isolation boundary, so resize and appearance require bounded source-checked messages. Catalog labels are declarations, not verified source facts. No-JavaScript mode shows ordinary navigation and complete reports rather than pretending app routes work. A SaaS-style interface does not constitute a hosted SaaS service.

## Verification Strategy

Build and render real retained inputs; exercise all routes, metadata search including no results, run filters, project/history selection, per-history restoration, Back/reload, light/dark/compact appearance, missing selections, offline desktop/mobile and capability fallback. Check sandbox origin and bounded rendering; run existing dashboard/workspace tests and lint. Record actual commands, screenshots and independent lifecycle/release holds without accepting or publishing specs.
