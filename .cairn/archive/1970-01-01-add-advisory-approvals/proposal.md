## Why

GitHub issue #8. `voter()` in `kiln_core::change_review` rejects any vote unless the actor is an eligible, requested, non-author reviewer, and the checklist counts only those eligible approvals on the current revision. A read principal who has looked at a change can only comment; there is no attributable, clearly non-binding "looks good to me". More eyes on a change should not widen merge authority, and advisory approvals are a useful signal for the review-friction observation views.

## What Changes

- **Domain:** separate `Command::AdvisoryApprove` and `Command::AdvisoryWithdraw` (not a new `VoteValue`), recorded in a separate `Change.advisory` collection as `{ actor, revision, created_ms }`, bound to the exact current revision. The change owner and revision authors may not advise on their own change. A new revision makes prior advisory approvals historical.
- **Authority boundary:** advisory approvals are invisible to the checklist, ownership, veto clearing, CI, candidates and queue admission; they do not clear or transfer to a candidate, add no attention, notifications or delivery recipients, and grant no write capability.
- **Shell:** any principal with read access may send exactly the two advisory actions; every other command still requires write access.
- **Console:** a separate labelled "Advisory approvals (non-binding)" list on the change detail with cast/withdraw through the existing confirmation dialog, historical counts, and an advisory search filter (also stored in saved views).
- **Observation:** the review-window export carries `advisory_approvals` per change (latest admitted patch set), and the review-friction view reports advisory totals per authorship class. The HTTPS and iroh observation shells and the dashboard render them identically.
- **Compatibility:** all new fields are serde-default and omitted when empty, so existing journals, saved views, review windows and view identities replay byte-identically.

## Impact

- **Consumers:** read principals who want to signal review without merge authority; change owners; operators reading review-friction views.
- **Owners:** Kiln maintainers (`kiln-core` change_review, `kiln-observe-core`); Kiln web-service maintainers (shell, console); observation adapter and dashboard maintainers.
- **Affected areas:** `crates/kiln-core/src/change_review.rs`, `crates/kiln-web-service/src/code_review*.rs`, `code-review.js`/`.css`, `crates/kiln-observe-core`, the HTTP and iroh observation adapter tests, `crates/kiln-dashboard`, `fixtures/observation`, `docs/operator.md`.

## Out of Scope

- Any path by which advisory approvals satisfy required approvals, ownership, veto clearing, CI or queue admission.
- New delivery channels or notifications for advisory approvals.
- Advisory approvals by principals without read access, or on closed changes.
- Changing eligible reviewer vote semantics.
