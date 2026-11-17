## Why

Issue [#47](https://github.com/OnixResearch/kiln/issues/47) records a residual in the selected-campaign composition. When native Cairn reports a selected change as completion-only, `stage_for_tasks` blocks the member because no implementation unit may be scheduled. A continuing campaign keeps the progress it already observed, but a campaign freshly selecting an already implemented change has no way to bind the canonical evidence of that implementation. The member stays blocked even when Kiln holds verified, integrated work for every unit, so its lifecycle step can never run.

## What Changes

- A completion-only member adopts existing canonical evidence when, for every required native unit, the current canonical work's latest attempt is independently verified with a passing authoritative Cairn completion check, is integrated through the selected delivery route for that exact work, attempt and fence, the integrated result is contained in the current target, and the change source is unchanged since that result.
- The member binds the attempt whose integrated result contains every other one, at its integrated stage, with the same work, attempt, freshness, check and evidence facts the verification and delivery owners record. Adopted units leave the member's remaining obligations. The existing lifecycle effect then runs without a claim.
- Each missing or stale fact keeps the member blocked with a reason naming it. No coding attempt is claimed or dispatched to manufacture lifecycle work.

## Capabilities

### Modified Capabilities

- `implementation-campaign`: completion-only members adopt current integrated evidence.

## Impact

`kiln-web-service` campaign observation (a new pure adoption decision, its Git applicability shell and the refresh wiring); delivery evidence helpers become shared within the campaign module. No store schema, campaign contract or provider protocol changes.

## Non-goals

Trusted preparation of lifecycle source changes in an isolated candidate, a separate durable lifecycle-only work kind, typed Cairn dependency milestones (Cairn #14, Kiln #83) and live qualification (#49) are not part of this change. Partial adoption is not offered: a change whose units are only partly implemented through Kiln stays blocked.
