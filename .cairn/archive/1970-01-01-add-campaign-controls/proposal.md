## Why

Issue [#86](https://github.com/OnixResearch/kiln/issues/86): an operator can watch a selected campaign in the console but cannot operate it there. The campaign HTTP projection is GET-only. Starting a campaign, continuing or cancelling admission, and answering a decision require the `kiln-implementer campaign` command-file CLI on the host. The console shows decision requests only as raw JSON. The campaign operator has no pause command, its commands carry no request identity, so a retried command after a lost response fails as stale instead of reconciling, and a decision answer is accepted after the member's work, policy or candidate moved on.

## What Changes

- **Owner commands.** Add a durable operator command record to the canonical campaign database. One immediate transaction applies a typed intent (start, continue, pause, cancel, issue decision, resolve decision) and records its request ID, actor, intent fingerprint and outcome. A repeated request with the same fingerprint returns the recorded outcome without a second transition; a reused ID with another intent or actor is a conflict. Add `pause` to the campaign operator command set.
- **Decision scope.** Resolving a decision refuses when the member's current work, policy or candidate no longer matches the request, using the same scope rule that admits the request.
- **Control profile.** Add `kiln-web-service serve --campaign-controls FILE`, a Nickel-checked profile naming the campaign database, per-principal grants (campaigns, commands, review roles, optional expiry) and start definitions (a frozen contract file and its admitted delivery route). The service rereads the profile for every control request, so removing a grant revokes it without a restart.
- **Routes.**
  - `GET /api/implementation/campaign-definitions` returns start previews.
  - `GET /api/implementation/campaigns/inbox?campaign=ID` returns the decision inbox and the operations open to the caller.
  - `POST /api/implementation/campaigns/commands` accepts typed intents bound to an expected version and a request ID. Stale versions, changed definitions and changed decision scope return refreshable conflicts; missing or expired grants and the wrong review audience are refused before any effect.
- **Console.** The Workers campaign panel shows start previews, admission controls and a decision inbox with answer forms, only for operations the service lists for the caller.
- **Exploration.** The disposable exploration workspace gains a control profile, decisions and definitions over its synthetic campaign.

## Impact

- **Consumer:** operators of selected campaigns; [#84](https://github.com/OnixResearch/kiln/issues/84)'s workspace later reuses the routes. Accepted transitions wake the continuation from [#87](https://github.com/OnixResearch/kiln/issues/87).
- **Immediate outcome:** start, continue, pause, cancel and decision answers from the console through the canonical owner, with idempotent retries and refreshable conflicts.
- **Durable capability:** one owner-side command record and typed intent path shared by the console, the CLI and later authority bindings.
- **Owner:** Kiln implementation and web-service maintainers.
- **Files:** `crates/kiln-core/src/implementation/campaign.rs`, `crates/kiln-implementer/src/{campaign.rs,campaign/commands.rs,campaign/command_tests.rs,campaign_operator.rs}`, `crates/kiln-web-service/src/{campaign_controls.rs,lib.rs,main.rs,server.rs,implementation_campaign/delivery.rs,inventory.js,inventory.css,workers.js}`, `crates/kiln-web-service/examples/{explore.rs,explore/campaign.rs}`, `config/web-campaign-controls.ncl` and fixtures, `tests/inventory-ui.test.mjs`, `docs/operator.md`, `docs/implementation-runbook.md`, `CHANGELOG.md`, `cairn-policy/development-coverage.ncl` and this package.
- **Verification:** core, implementer and web-service tests over real SQLite stores and control profiles; Node UI tests; Nickel fixtures; browser review of the exploration workspace; workspace and Nix gates; Cairn gates, validation and traceability.
- **Authority:** working-copy implementation and verification only. No deployment, live campaign or real worker effect.

## Out of Scope

Holder-bound Basalt/UCAN authority and delegation ([#69](https://github.com/OnixResearch/kiln/issues/69)/[#70](https://github.com/OnixResearch/kiln/issues/70)); grants here are profile-listed principals. Contract revision and scope expansion through the console; a changed selection still needs the owner's explicit revision command. The unified Cairn workspace ([#84](https://github.com/OnixResearch/kiln/issues/84)) and typed Cairn snapshots ([#83](https://github.com/OnixResearch/kiln/issues/83)). Experiment admission for empirical-uncertainty decisions. Deployment qualification ([#49](https://github.com/OnixResearch/kiln/issues/49)).
