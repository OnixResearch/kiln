# Implementation campaign delta

## ADDED Requirements

### Requirement: Completion-only members adopt current integrated evidence

r[kiln.implementation.campaign.adoption] A selected member whose native Cairn snapshot reports completion-only MUST NOT schedule coding work. Kiln MUST adopt existing canonical evidence only when, for every required native unit, the current canonical work's latest attempt is bound to that work, is independently verified with exactly one passing authoritative Cairn completion check, and has integration evidence from the selected delivery route for that exact work, attempt and fence; the integrated result MUST be contained in the current target and the change source MUST be unchanged since that result. The member MUST then bind the attempt whose integrated result contains every other one, at its integrated stage, and its adopted units MUST leave its remaining obligations. Otherwise the member MUST stay blocked with a reason naming the missing or stale fact.

#### Scenario: Current integrated evidence is adopted

r[kiln.implementation.campaign.adoption.scenario.adopted]
- GIVEN a freshly selected completion-only member whose required units each have verified, completion-bearing, integrated canonical evidence contained in the current target with an unchanged change source
- WHEN native observation runs
- THEN the member MUST bind the attempt whose integrated result contains the others, at its integrated stage, without a claim or a coding dispatch

#### Scenario: Missing or stale evidence stays an explicit blocker

r[kiln.implementation.campaign.adoption.scenario.blocked]
- GIVEN a completion-only member with a unit lacking current canonical work, an attempt bound to another work version, an unverified attempt, no single passing completion check, no bound integration, an integrated result the target does not contain, a change source moved after integration, or diverging integrated results
- WHEN native observation runs
- THEN the member MUST stay blocked with a reason naming that fact, and no attempt MUST be claimed or bound
