## Why

After the layout restructure the operator asked for a page-by-page pass to fix remaining UI/UX problems. The shell, overview and run view were restructured, but most secondary pages were only re-framed. Examples: projects, workers, activity, evidence, automation, operations, settings, source, receipts, COBs, the run tabs, planning, inbox, triage, saved views and the code-review detail. They still carry dense helper prose, inconsistent spacing and controls, raw identifiers as primary text, and uneven empty, loading and error states.

## What Changes

- Audit every live-console page in a real browser on real data at 320, 768, 1440 and 1920px, in light and dark.
- Fix the issues found on each page:
  - clear hierarchy and one primary action;
  - consistent shared components;
  - readable primary text, with exact identifiers secondary but inspectable;
  - concise copy, with boundary and non-claim text kept but moved behind progressive disclosure where it currently dominates;
  - explicit and consistent empty, loading and error states;
  - no overflow or truncation that hides meaning;
  - keyboard and touch accessibility.
- Retain before/after evidence per page.

## Impact

Consumers are authenticated Kiln developers and CI operators. Kiln web-service maintainers own the pages. Durable contribution: each page conforms to the shared frame and component system, so later work changes data, not layout. Affected: `crates/kiln-web-service/src` web assets, tests or docs that assert changed strings, and operator documentation.

## Out of Scope

No API, storage, authority, route, identity or provider changes. No removal of required non-claim or boundary statements; they may move into disclosures but must remain reachable. No new remote assets. No fabricated data.
