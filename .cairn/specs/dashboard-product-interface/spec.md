# Dashboard Product Interface Specification

## Purpose

Defines the `dashboard-product-interface` capability.

## Requirements

### Requirement: Coherent read-only application shell
r[kiln.dashboard.product.shell]
The catalog MUST offer one responsive SaaS-style application shell with Overview, Projects, Runs, Activity, Evidence and Settings, using real admitted data and the actual Rats index, without fabricated account, billing or CI controls. Counts MUST be scoped to catalog inventory or one selected independent history, not summed overlapping journals.

#### Scenario: Real catalog inventory
r[kiln.dashboard.product.shell.scenario.inventory]
- GIVEN an admitted catalog with multiple histories
- WHEN Overview or Projects is opened
- THEN declared project/history inventory MUST be readable and an explicit history selection MUST open only that admitted report.

### Requirement: Working restorable navigation and preferences
r[kiln.dashboard.product.navigation]
Routes, metadata search, exact project/history selection and appearance settings MUST work offline and survive reload via inert URL state; Back MUST restore prior navigation. Missing selections MUST remain missing and labels MUST remain inert.

#### Scenario: Search and missing restoration
r[kiln.dashboard.product.navigation.scenario.restore]
- GIVEN a metadata search, a saved route or a removed selected history
- WHEN the user searches, reloads or navigates Back
- THEN matching inventory or an explicit empty state MUST appear, route and selections MUST restore, and unavailable identities MUST NOT silently select a replacement.

### Requirement: Isolated embedded report presentation
r[kiln.dashboard.product.isolation]
Embedded reports MUST retain opaque-origin sandbox isolation and canonical view facts. The parent/child bridge MUST accept route and appearance messages only from its associated window and MUST NOT grant provider authority, parent DOM access or external resource access. Standalone reports MUST remain usable.

#### Scenario: Embedded route changes
r[kiln.dashboard.product.isolation.scenario.route]
- GIVEN a selected independently admitted report
- WHEN Runs, Activity, Evidence or appearance is selected
- THEN the matching bounded facts MUST appear without duplicate application chrome, preserving run/attempt state and identities.

### Requirement: Exercised responsive offline behavior
r[kiln.dashboard.product.behavior]
The actual generated artifact MUST be exercised at desktop and narrow widths, offline, with search/route/selection/restoration and unavailable-capability cases. No-JavaScript navigation and complete reports MUST remain readable. Existing output and admission bounds MUST remain enforced.

#### Scenario: Unavailable browser capability
r[kiln.dashboard.product.behavior.scenario.fallback]
- GIVEN blocked network or unavailable JavaScript or WebAssembly
- WHEN the local artifact is opened
- THEN supported offline UI MUST work, unavailable capability MUST be visible and accessible navigation and reports MUST remain available.

### Requirement: Bounded product and delivery claims
r[kiln.dashboard.product.scope]
Documentation and evidence MUST identify exercised behavior, maintenance owner and repeatability. Delivery MUST remain a separate finite preview without changing the live publisher. UI success MUST NOT imply hosted accounts, CI success, freshness, release readiness, specification acceptance or publication.

#### Scenario: Verified preview delivery
r[kiln.dashboard.product.scope.scenario.delivery]
- GIVEN a verified generated preview and the enrolled Aspen3 destination
- WHEN finite preview delivery completes
- THEN remote preview bytes MUST match local bytes and the live index MUST remain unchanged.
