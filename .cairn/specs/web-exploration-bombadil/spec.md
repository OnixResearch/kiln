# Web Exploration Bombadil Specification

## Purpose

Defines the `web-exploration-bombadil` capability.

## Requirements

### Requirement: Property-based exploration of the live website
r[kiln.web.exploration.browser_property_rail]
Kiln MUST provide a reusable bounded Bombadil browser rail over the current real example console, with native context, meaningful UI-state properties, safe read-only interactions, retained native outcomes and explicit evidence boundaries. It MUST NOT substitute a static report, mock page, no-op property or unchecked tool launch for actual browser exploration.

#### Scenario: Reuse the pinned separately executed cohort
r[kiln.web.exploration.browser_property_rail.scenario.cohort]
- GIVEN the existing Bombadil input and package convention
- WHEN the live website rail is configured
- THEN it MUST use the same exact tool revision as a separate program, expose repeatable commands without adding production crate dependencies, and leave the tool pin and lockfile unchanged

#### Scenario: Observe consumer-visible browser state
r[kiln.web.exploration.browser_property_rail.scenario.properties]
- GIVEN an admitted native example catalog and a real rendered console
- WHEN bounded read-only actions explore routes, filters, history, run tabs and viewport states
- THEN the rail MUST evaluate semantic search/selection/readiness/layout properties and standard HTTP/JavaScript error properties against observed state, without calling application internals or masking a violation

#### Scenario: Preserve target and lifecycle authority
r[kiln.web.exploration.browser_property_rail.scenario.isolation]
- GIVEN a default-owned workspace or an explicitly selected attach URL
- WHEN a campaign starts, fails, times out or finishes
- THEN a non-example target MUST be refused before exploration, navigation/actions MUST remain within admitted read-only example scope, owned children MUST be cleaned up, and attached CI/example services, credentials and ingress MUST remain unchanged

#### Scenario: Retain bounded and honest run evidence
r[kiln.web.exploration.browser_property_rail.scenario.evidence]
- GIVEN an actual pinned Bombadil campaign
- WHEN Kiln records its outcome
- THEN it MUST preserve the native command, tool version, context, trace and success/violation/error outcome and inspect useful property/action evidence, without claiming exhaustive exploration, replay across reset identities, CI/provider correctness, production qualification or release authority
