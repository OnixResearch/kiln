## Why

The operator asked for a UI/UX redesign to modern design practice while keeping Kiln's off-white/charcoal and acid-yellow identity. The live console is functional but dated. Raw outcome enums ("DomainFailure"), millisecond-precision durations and absolute-only timestamps make scanning slow. Boxy hard-bordered controls give every button equal weight, pages repeat Refresh actions, and session controls look like primary actions. Run detail and work pages spend space on chrome instead of observations.

## What Changes

- Replace ad-hoc styling with a documented token system covering spacing, type scale, radii, elevation, motion and semantic status colors. Keep the Kiln palette, light/dark/system appearance and compact density.
- Introduce shared presentation components: status pills with icon, human label and exact outcome; humanized durations and relative times with exact values inspectable; data tables with sticky headers; page headers with one primary action; loading skeletons; and empty or error states.
- Restyle the shell, CI views (overview, projects, runs, run detail, logs, workers and inspection/administration pages), work views (issues, inbox, planning, triage, saved views) and code review on that system.
- Keep every route, identity, authority, confirmation, keyboard and accessibility contract. Respect reduced motion and keep WCAG AA contrast.

## Impact

Consumers are authenticated Kiln developers and CI operators using the live console. Kiln web-service maintainers own the design system. Durable contribution: one reusable token and component layer, so future views stop inventing local styles. Affected: `crates/kiln-web-service/src` assets, a shared UI helper asset served by the existing static route table, and operator documentation where labels change.

## Out of Scope

No API, storage, execution, authority, identity, publication or provider changes. No remote fonts, icon CDNs, analytics or new runtime dependencies. The offline HTML report and the optional hosted bridge UI are unchanged. No fabricated observations: unknown timing remains unknown.
