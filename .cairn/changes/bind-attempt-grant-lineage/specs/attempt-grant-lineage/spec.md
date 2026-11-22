# Attempt grant lineage delta

## ADDED Requirements

### Requirement: Root binding to the current attempt

r[kiln.attempt.grant_lineage.root] Kiln MUST bind an admitted standing-policy ceiling to the exact current campaign revision, project, work, work version, attempt and fence, the holder and incarnation, the root policy reference and one owner-confirmed #61 budget reservation before any grant for that attempt is issued. A ceiling from another authority generation, an expired ceiling, a ceiling for another project (including a project-wide ceiling) or scoped to other work, or a reservation the budget owner did not confirm MUST be refused.

#### Scenario: Root admission refuses moved, expired, foreign or unbudgeted ceilings

r[kiln.attempt.grant_lineage.root.scenario.refused]
- GIVEN an admitted ceiling and the current attempt facts
- WHEN the authority generation moved, the ceiling expired, the ceiling names another project or other work, or no or another reservation is confirmed
- THEN admission MUST refuse with `stale_generation`, `outlives_parent`, `foreign_lineage` or `budget_unbound`

### Requirement: Child attenuation

r[kiln.attempt.grant_lineage.attenuation] An admitted parent binding MUST derive a child only when the verified delegator holds the parent, holds explicit delegation permission and the parent ceiling permits a further hop. The child ceiling MUST NOT exceed the parent's audience, project, work scope, disclosure routes, operations, lifetime or remaining delegation depth, and a child MUST hold exactly one role that its parent holds: coding authority MUST NOT derive verification, publication, integration or lifecycle authority. Every refusal MUST be one closed `LineageRefusal` reason, chosen in a fixed check order.

#### Scenario: Permitted child and grandchild derive

r[kiln.attempt.grant_lineage.attenuation.scenario.permitted]
- GIVEN an admitted root with delegation depth two
- WHEN its holder delegates a narrower coding child and that child delegates a narrower leaf
- THEN both MUST be admitted with depth, parent, lineage and policy inherited and their ceilings covered by their parents

#### Scenario: Every widening is refused

r[kiln.attempt.grant_lineage.attenuation.scenario.refused]
- GIVEN an otherwise admitted child request
- WHEN exactly one input widens resource, operation, audience, lifetime or depth, lacks delegation permission, escalates role, is foreign, is unbudgeted or is stale
- THEN admission MUST refuse with the matching `LineageRefusal` and every refusal variant MUST have such a fixture

#### Scenario: No admitted child covers what its parent does not

r[kiln.attempt.grant_lineage.attenuation.scenario.property]
- GIVEN every operation subset as a parent and as a child, and every combination of lifetime, depth, route set, work, project and audience
- WHEN each child is requested
- THEN a child MUST be admitted exactly when it holds one role and no operation outside its parent, and every admitted child ceiling MUST be covered by its parent ceiling

### Requirement: Foreign lineage cannot enter

r[kiln.attempt.grant_lineage.foreign] A parent binding or grant reference for another campaign revision, project, work, work version, attempt or holder, a binding whose identity does not recompute, and a chain whose links do not recompute, link, attenuate, keep one inherited role and derive their budget root first MUST be refused as `foreign_lineage`. Lineage comparison MUST check campaign revision, project, work and version before the fence and the fence before the attempt identity, so another work with a new fence is foreign and a replacement attempt with a new fence is stale.

#### Scenario: Copied or foreign parent is refused

r[kiln.attempt.grant_lineage.foreign.scenario.refused]
- GIVEN a valid parent for one attempt
- WHEN the current attempt is another campaign, project, work, version or attempt, the delegator is not the holder, the parent was edited, a recomputing link is wider, deeper, under another policy, mixes roles or charges an unrelated budget, or its ancestors are missing or out of order
- THEN derivation and currentness MUST refuse as `foreign_lineage`

### Requirement: Budget binding

r[kiln.attempt.grant_lineage.budget] Every grant MUST bind one #61 reservation that the budget owner confirmed. A child MUST either share its parent's account or charge a disjoint account transferred from the parent's account. The issuance fold MUST refuse to reserve one account from two parent budgets.

#### Scenario: Unbound or re-pointed reservation is refused

r[kiln.attempt.grant_lineage.budget.scenario.refused]
- GIVEN a child request
- WHEN no reservation is confirmed, the request differs from the confirmed reservation, or the account is not transferred from the parent's account
- THEN admission MUST refuse as `budget_unbound`, and the issuance fold MUST refuse an account already reserved from another parent budget

### Requirement: Lineage currentness

r[kiln.attempt.grant_lineage.currentness] A binding MUST be current only while every link in its lineage carries the current authority generation, was admitted at or before the observed revocation generation, has an unrevoked proof reference, is unexpired, and binds the current attempt and fence. Revocation or expiry of any ancestor MUST invalidate every descendant, and the issuance fold MUST propagate a revoked grant to its issued and pending descendants.

#### Scenario: Revocation and expiry propagate

r[kiln.attempt.grant_lineage.currentness.scenario.propagate]
- GIVEN a root, child and grandchild
- WHEN any lineage proof is revoked, any link expires, or a later fence replaces the attempt
- THEN currentness MUST refuse the grandchild, an issued descendant MUST be revoked and a pending descendant MUST be abandoned

### Requirement: Deterministic identities

r[kiln.attempt.grant_lineage.identity] Ceiling, intent and binding identities MUST be BLAKE3 over a domain-separated, length-framed encoding of every other field. The intent identity MUST exclude the proof reference, so the stored intent stays the same identity before and after minting and recovery resumes it rather than re-deriving one from fresh facts; the binding identity MUST include it. A binding MUST carry proof references only and refuse unknown fields such as token bytes.

#### Scenario: Same inputs give the same identity

r[kiln.attempt.grant_lineage.identity.scenario.deterministic]
- GIVEN one admitted request
- WHEN it is admitted again or any bound field changes
- THEN the same inputs MUST yield the same intent and binding, any changed field MUST change the identity, and an edited binding MUST fail validation

### Requirement: Attempt retains its binding

r[kiln.attempt.grant_lineage.attempt] An implementation attempt MUST retain at most one authority binding, absent from serialization until bound. A first binding MUST require a recomputing binding for exactly this attempt's work, version, identity and fence while the attempt holds the current, unexpired claim; repeating the same binding MUST be idempotent and an alternative binding MUST be refused.

#### Scenario: One exact binding per attempt

r[kiln.attempt.grant_lineage.attempt.scenario.bound]
- GIVEN a claimed attempt
- WHEN its exact binding is bound twice, an alternative proof is offered, another attempt's binding is offered, the lease expired, a successor replaced it while it is still claimed, or the attempt was cancelled
- THEN the first MUST succeed idempotently and round-trip, and the others MUST be refused

### Requirement: Issuance intents reconcile once

r[kiln.attempt.grant_lineage.issuance] Issuance MUST be journaled as an intent before minting and an issued proof reference after. Folding records MUST perform no effect. Reconciliation MUST resolve each intent to the current attempt lineage of its own work unit and resume an unissued intent under its stored identity only while it matches the current authority generation, binds exactly that resolved lineage, and its attempt holds the current, unexpired claim at its fence in the attempt ledger; an intent whose work unit has no current attempt, or whose lease lapsed even before expiry processing, MUST be abandoned. It MUST never issue one intent twice or one proof for two intents, and never mint under a revoked ancestor, after expiry, after a generation move or for a replaced or foreign attempt.

#### Scenario: A crash resumes the original intent

r[kiln.attempt.grant_lineage.issuance.scenario.recover]
- GIVEN an intent recorded without an issued proof, including a duplicated intent record
- WHEN the journal is replayed
- THEN reconciliation MUST owe exactly one issuance under that intent while it is current and abandon it once the generation or its work unit's lineage moved, the work unit has no current attempt, or the attempt's lease lapsed, intents of several current attempts MUST all be issued in one reconciliation, a second proof for one intent MUST be refused, and a fully issued journal MUST owe nothing
