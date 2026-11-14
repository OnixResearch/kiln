## ADDED Requirements

### Requirement: Usable contained console topbar
r[kiln.web.topbar_containment]
The console topbar MUST remain within the document viewport while preserving useful location, account, search and live-state controls. Verifying a header fix MUST NOT weaken the existing Bombadil viewport property or grant host-maintenance or deployment authority.

#### Scenario: Long location within the available header
r[kiln.web.topbar_containment.scenario.location]
- GIVEN the long Raw collaboration history location and the synthetic operator identity
- WHEN the console is rendered at 390, 768, 1024 and 1440px, with the desktop rail collapsed or pinned where supported
- THEN no document-level horizontal overflow MUST occur, and the location MUST retain a visible nonzero region rather than collapsing to zero

#### Scenario: Controls remain usable when labels yield space
r[kiln.web.topbar_containment.scenario.controls]
- GIVEN a constrained header and full accessible names for account and search controls
- WHEN the operator opens and dismisses the account menu or command palette with pointer or keyboard controls
- THEN those controls MUST remain reachable, mobile targets MUST retain their existing 44px minimum, and the fix MUST NOT mask overflow by hiding the document or discarding navigation content

#### Scenario: Read-only host diagnosis before bounded builds
r[kiln.web.topbar_containment.scenario.host_safety]
- GIVEN independently configured host automatic-GC thresholds and constrained shared storage
- WHEN this change diagnoses policy and rebuilds the selected example
- THEN it MUST record source-backed threshold/capacity observations, disable automatic GC per Nix invocation, and MUST NOT change host policy or perform shared cleanup, deployment or managed-service restarts without separate operator authorization

#### Scenario: Retained unchanged browser-rail evidence
r[kiln.web.topbar_containment.scenario.evidence]
- GIVEN rebuilt local assets and an explicitly selected existing deployed example
- WHEN the bounded Bombadil rail runs
- THEN it MUST use the unchanged semantic properties, retain native outcomes and meaningful observed states, clean up owned processes and preserve managed services, without treating a deployed observation as deployment of this local fix or claiming exhaustive coverage or release authority
