## Why

Issue [#85](https://github.com/OnixResearch/kiln/issues/85): a selected campaign stops all work when one member waits for review. `CampaignState::next_action` sorts candidates and returns the first decision it derives. When member A sorts before C and its verified candidate awaits an outstanding required review, selection returns A's wait. Independent, ready member C is never considered. The bounded driver stops when the selected decision has no effect, so C makes no progress until A's review is answered.

Missing required context, stale inputs, member-local replan, unobserved delivery capability and missing endpoint evidence have the same effect. Member-key order decides whether unrelated work proceeds.

## What Changes

- Evaluate every selected member into an admissible owner action, a hold in its own scope, or a deferred wait on a dependency, the shared writer slot or observation. Select the first member action in the existing deterministic order. Surface a member hold only when no member can act, and retain every hold beside the selection (`CampaignState::evaluate`, `CampaignEvaluation`, `SelectionBasis`). `next_action` returns the same selection.
- Keep campaign barriers ahead of member work: unapplied or uncertain effects, cancellation, completion, pause and bounded decision issuance. Writer claims (`AuthorArtifact`, `RepairArtifact`, `Code`, `RepairCode`, `RetryInfrastructure`) wait while another active attempt holds the repository's writer slot. Canonical admission still makes the final claim, budget and lineage decision.
- Move the native controller's bounded loop to `kiln_implementer::campaign::drive_bounded` over a `CampaignDriveShell`. Its refresh, ambiguous-dispatch recovery and canonical-claim steps stay native. When no member can act, check every executing member for an ambiguous dispatch, so one member's surfaced hold does not defer another member's recovery. The drive output adds `basis` and `holds`.
- The selected-campaign projection adds `selection_basis` and each member's `selection_hold`, and the console renders the hold. Every waiting member stays visible while an independent member is next.
- The synthetic exploration campaign gains an independent ready member, so the example shows a proposed action beside member holds. Its catalog link now uses the Workers page's `worker_campaign` route parameter; the previous `campaign` parameter opened the page without loading the campaign.

## Impact

- **Consumer:** operators running `kiln-web-service drive-implementation-campaign` and reading the selected-campaign projection.
- **Immediate outcome:** independent ready work in a selected campaign advances while another member awaits review, context, replan or delivery capability. Each waiting member keeps its exact reason.
- **Durable capability:** a pure member evaluation with retained explanations, and one tested bounded loop shared by the native controller and scripted owners.
- **Owner:** Kiln implementation and web-service maintainers.
- **Files:** `crates/kiln-core/src/implementation/campaign.rs`, `crates/kiln-implementer/src/campaign.rs`, `crates/kiln-implementer/src/campaign/drive_tests.rs`, `crates/kiln-web-service/src/{implementation_campaign.rs,campaign_projection.rs,inventory.js}`, `crates/kiln-web-service/examples/{explore.rs,explore/campaign.rs}`, `fixtures/selected-campaign/projection-v1.json`, `tests/inventory-ui.test.mjs`, `cairn-policy/development-coverage.ncl`, `docs/{implementation-runbook.md,operator.md}`, `CHANGELOG.md` and this package.
- **Verification:** core, implementer and web-service unit tests; the exploration example tests; the inventory Node rail; formatting and Clippy for touched crates; a browser check of the example campaign view; Cairn gates, validation and active-delta traceability.
- **Authority:** working-copy implementation and verification only. No real worker, model, credential, provider or remote effect, and no deployment, publication or lifecycle acceptance.

## Out of Scope

Concurrent writers, relaxed review or check gates, new admission or budget policy, a campaign claim table, and the existing driver error for a member-level unknown-outcome reconcile that has no retained campaign effect. Budget exhaustion remains a canonical-admission decision.
