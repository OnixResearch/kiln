## Phase 1: Integration

- [x] [serial] Validate the native planning gates and reuse the existing pinned tool/API without changing lockfiles or production dependencies. r[kiln.web.exploration.browser_property_rail.scenario.cohort]
- [x] [parallel] Implement read-only live-console browser properties and safe action generators over staged native context. r[kiln.web.exploration.browser_property_rail.scenario.properties]
- [x] [parallel] Implement owned-default/explicit-attach campaign lifecycle, context admission, trace retention and native outcome propagation. r[kiln.web.exploration.browser_property_rail.scenario.isolation] r[kiln.web.exploration.browser_property_rail.scenario.evidence]
- [x] [serial] Integrate tool/runner flake apps and a dedicated browser-testing environment using existing package conventions. r[kiln.web.exploration.browser_property_rail.scenario.cohort]

## Phase 2: Exercise

- [x] [serial] Run actual pinned Bombadil campaigns over a fresh native example and the deployed read-only tailnet origin; inspect trace/property/action evidence and diagnose violations. r[kiln.web.exploration.browser_property_rail.scenario.properties] r[kiln.web.exploration.browser_property_rail.scenario.evidence]
- [x] [serial] Verify refusal of a non-example target, owned cleanup and preservation of attached services; document commands, owner, results and non-claims and run focused lifecycle/coverage checks. r[kiln.web.exploration.browser_property_rail.scenario.isolation] r[kiln.web.exploration.browser_property_rail.scenario.evidence]
