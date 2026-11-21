# Convergence Campaign Specification

## Purpose

Defines the `convergence-campaign` capability.

## Requirements

### Requirement: Bounded admission of a Cairn convergence receipt

r[kiln.convergence.intake] Kiln MUST admit only a complete
`cairn.convergence.v1` receipt whose schema, status, row vocabulary, named byte
and count bounds and identity binding hold. Kiln MUST recompute the receipt's
BLAKE3 from its canonical JSON without the `receipt_blake3` field and MUST
compare it with both that field and the declared intake binding. The intake
MUST bind the receipt BLAKE3, the registry revision kind and value, the
registry, case and policy BLAKE3, and MUST reject a receipt for another
revision or identity. Kiln MUST reject an unknown schema, an unknown receipt
field or row field, a malformed identity, an oversized receipt, an unknown row
state, a duplicate requirement row and an unbound-state row set before any
campaign decision.

#### Scenario: The retained pilot receipt is admitted

r[kiln.convergence.intake.scenario.pilot]
- GIVEN the retained `cairn.convergence.v1` pilot receipt bytes and its declared binding
- WHEN pure intake runs
- THEN it MUST admit every typed row and return the receipt identity, registry revision, registry, case and policy BLAKE3

#### Scenario: An unknown schema or field is rejected

r[kiln.convergence.intake.scenario.schema]
- GIVEN a receipt with another `schema_version`, an unknown top-level field, an unknown row field or an unknown row state
- WHEN pure intake runs
- THEN it MUST reject the receipt before selection

#### Scenario: A stale or substituted identity is rejected

r[kiln.convergence.intake.scenario.identity]
- GIVEN a receipt whose recomputed canonical BLAKE3 differs from `receipt_blake3`, or whose declared binding names another receipt, registry revision, registry, case or policy identity
- WHEN pure intake runs
- THEN it MUST reject the receipt without admitting any row

#### Scenario: Oversized input is rejected without parsing

r[kiln.convergence.intake.scenario.oversized]
- GIVEN bytes longer than the named receipt bound
- WHEN pure intake runs
- THEN it MUST reject the input before JSON parsing

#### Scenario: A current row is not Kiln acceptance

r[kiln.convergence.intake.scenario.current]
- GIVEN an admitted receipt whose rows are `current` and whose `complete` flag is true
- WHEN intake returns
- THEN it MUST NOT set any Kiln gate, run, attempt or candidate outcome

### Requirement: Unsatisfied evidence selection

r[kiln.convergence.unsatisfied] Kiln MUST select the unsatisfied evidence set
from admitted rows as exactly those targeted rows whose state is `unrealized`,
`stale`, `failed`, `unsupported` or `incomplete`. It MUST exclude `current`,
`not_projected` and `untargeted` rows and MUST retain the exclusion reason for
each excluded state. Selection MUST preserve registry row order and MUST retain
each selected row's requirement, specification path, row BLAKE3 and state.

#### Scenario: Only non-current targeted rows are selected

r[kiln.convergence.unsatisfied.scenario.selection]
- GIVEN an admitted receipt with `current`, `not_projected`, `untargeted`, `unrealized`, `stale`, `failed`, `unsupported` and `incomplete` rows
- WHEN unsatisfied selection runs
- THEN it MUST select the five non-current targeted states in registry order and exclude the other three with their stated reason

#### Scenario: A moved or unbound target is selected

r[kiln.convergence.unsatisfied.scenario.moved]
- GIVEN a row whose binding is missing or names another revision
- WHEN the row state is `stale` or `unrealized`
- THEN selection MUST include the row rather than treat it as satisfied

#### Scenario: Satisfied evidence selects nothing

r[kiln.convergence.unsatisfied.scenario.satisfied]
- GIVEN a receipt whose targeted rows are all `current` or `not_projected`
- WHEN unsatisfied selection runs
- THEN the set MUST be empty and no campaign revision MUST be minted

### Requirement: One frozen campaign revision from the unsatisfied set

r[kiln.convergence.campaign] Kiln MUST mint at most one frozen evidence campaign
revision from one unsatisfied set and MUST bind it to exactly one candidate-set
BLAKE3 derived from the existing `CandidateSet` value, one receipt BLAKE3, one
registry revision, and the registry, case and policy BLAKE3. The revision
identity MUST be a BLAKE3 over a domain-separated encoding of those bindings
and the ordered items. Reordering the items, moving the candidate, changing a
row or binding, or repeating the mint MUST produce the same identity for
identical facts and a different identity for any changed fact. A minted
revision MUST be immutable and MUST NOT be minted twice for one receipt and
candidate.

#### Scenario: An unsatisfied set mints one revision

r[kiln.convergence.campaign.scenario.mint]
- GIVEN an admitted pilot receipt, a candidate-set BLAKE3 and a non-empty unsatisfied set
- WHEN campaign minting runs
- THEN it MUST return one revision whose items equal the selected set and whose identity is deterministic

#### Scenario: A changed candidate or row changes the identity

r[kiln.convergence.campaign.scenario.identity]
- GIVEN one minted revision
- WHEN the candidate-set BLAKE3, a row BLAKE3, a state or an item order changes
- THEN the minted identity MUST differ, and an unchanged repeat MUST be identical

#### Scenario: An empty set or duplicate item mints nothing

r[kiln.convergence.campaign.scenario.empty]
- GIVEN an empty unsatisfied set, an empty receipt row list, or a duplicate requirement identity
- WHEN campaign minting runs
- THEN an empty set MUST mint no revision, and an empty row list or duplicate identity MUST be refused

#### Scenario: A campaign item must name its evidence job

r[kiln.convergence.campaign.scenario.job]
- GIVEN a selected row without a contract or job identity
- WHEN campaign minting runs
- THEN it MUST refuse the campaign because the evidence job cannot be requested

### Requirement: Verify-stage typed evidence build requests

r[kiln.convergence.verify] For every campaign item the Verify stage MUST produce
one typed evidence build request bound to the campaign identity, item, candidate
identity, Mantle provider profile, plan identity, policy identity, platform and
sorted unique required products. Kiln MUST admit the request only when the
selected capability profile declares a build-role dispatch with the required
outcome classes, and MUST reject an undeclared operation, role or profile
before any effect. Kiln MUST admit a preserve or realize realization only when
the selected capability profile also declares the preserve realization
capability, and MUST reject a `BuildFromPlan` request against a profile that
declares that capability. Each refusal MUST use a stable code before any
effect. A request MUST lower to the existing `BuildEffectPlan` using the same
build admission rules.

#### Scenario: A campaign item produces one exact request

r[kiln.convergence.verify.scenario.request]
- GIVEN one minted campaign revision and a capability profile that declares build dispatch
- WHEN Verify request planning runs
- THEN each item MUST produce one deterministic request and the requests MUST lower to admitted build plans

#### Scenario: Unsupported capability fails before effects

r[kiln.convergence.verify.scenario.capability]
- GIVEN a capability profile without build dispatch, another role, another profile identity or a missing required outcome
- WHEN Verify request planning runs
- THEN it MUST reject before a request or effect exists

#### Scenario: Unsupported realization fails before effects

r[kiln.convergence.verify.scenario.realization]
- GIVEN a Verify selection that requests `cache-only-preserve-v1` and a profile that does not declare the preserve realization capability
- WHEN Verify request planning runs
- THEN it MUST reject with `unsupported_capability` before dispatch

#### Scenario: Realization and profile mismatch fails before effects

r[kiln.convergence.verify.scenario.realization_mismatch]
- GIVEN a `BuildFromPlan` selection against a profile that declares the preserve realization capability, or a `PreserveCacheOnly` selection against a profile that does not
- WHEN Verify request planning runs
- THEN it MUST reject with a stable code before a request or effect exists

#### Scenario: A missing or unexpected plan is refused

r[kiln.convergence.verify.scenario.plan]
- GIVEN a plan binding that omits a campaign job, names another job or supplies duplicate, unsorted or empty products
- WHEN Verify request planning runs
- THEN it MUST refuse without producing any request

### Requirement: Convergence campaign non-claims

r[kiln.convergence.nonclaims] An admitted receipt, an unsatisfied set, a minted
campaign revision or a planned Verify request MUST NOT prove requirement
correctness, evidence truth, build correctness, cache truth, reproducibility,
source honesty, release eligibility or that a provider performed an effect.
Cairn `current`, Mantle success and a cache hit MUST remain advisory to Kiln.

#### Scenario: Advisory evidence does not grant release

r[kiln.convergence.nonclaims.scenario.advisory]
- GIVEN a `complete` receipt and a planned Verify request
- WHEN Kiln records the campaign
- THEN no merge, publication, release or provider effect MAY follow from that record alone

### Requirement: Preserve-route evidence realization

r[kiln.convergence.realize] A campaign item whose selection declares
`PreserveCacheOnly` MUST realize the same frozen foreign derivation graph the
unsatisfied evidence job names through the pinned foreign-realize cohort, not
by building it again. Kiln MUST lower the request to the existing
`BuildEffectPlan` and resolve one realize profile binding by plan identity. It
MUST admit completion only from a closed-world realization receipt that binds
the same plan, import receipt, source bundle manifest and cache-closure policy,
selects exactly one root, and maps every required product to that root under
the logical store prefix.

#### Scenario: The pilot evidence job is realized

r[kiln.convergence.realize.scenario.pilot]
- GIVEN the retained phase-1 plan, import receipt, source bundle, execution profile, cache-closure policy and endpoint binding, and the pinned realize cohort
- WHEN the Verify request is dispatched through `kiln-adapter-mantle`
- THEN the adapter MUST admit completion with the pilot's selected root and the pilot's plan and import-receipt identities

#### Scenario: A substituted frozen input is refused

r[kiln.convergence.realize.scenario.substitution]
- GIVEN a plan, import receipt, source bundle, execution profile, policy or binding config whose identity differs from the admitted binding
- WHEN dispatch or completion runs
- THEN it MUST refuse before completion and MUST NOT redispatch into another realization
