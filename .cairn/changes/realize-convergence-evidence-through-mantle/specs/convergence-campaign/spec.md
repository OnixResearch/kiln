# Cairn Convergence Campaign

## MODIFIED Requirements

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

## ADDED Requirements

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
