## Phase 1: Safety and implementation

- [x] [parallel] Diagnose configured GC thresholds and source, explain the unexpected request, and record a safe no-cleanup build invocation and capacity. r[kiln.web.topbar_containment.scenario.host_safety]
- [x] [parallel] Fix intrinsic header overflow while preserving a usable breadcrumb and account/search controls. r[kiln.web.topbar_containment.scenario.location] r[kiln.web.topbar_containment.scenario.controls]

## Phase 2: Verification

- [x] [serial] Rebuild the selected example and exercise the original scenario at 390, 768, 1024 and 1440px, including pinned rail and account/search interactions. r[kiln.web.topbar_containment.scenario.location] r[kiln.web.topbar_containment.scenario.controls]
- [x] [serial] Run the unchanged bounded Bombadil rail against owned and deployed examples, retain native results, and verify teardown and protected service preservation. r[kiln.web.topbar_containment.scenario.evidence]
- [x] [serial] Document the fix, policy diagnosis, maintenance ownership and evidence boundaries; pass native lifecycle and active traceability checks without archiving or publishing. r[kiln.web.topbar_containment.scenario.evidence]
