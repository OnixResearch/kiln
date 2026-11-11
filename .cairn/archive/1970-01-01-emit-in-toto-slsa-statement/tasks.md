## Phase 1: Pure mapping

- [x] [serial] Add the canonical in-toto/SLSA statement mapping with distinct builder, attempt, provider, worker and control-plane fields. r[kiln.interchange.statement]
- [x] [serial] Refuse other-attempt projections, empty subjects and colliding native digest keys before serialization. r[kiln.interchange.statement.scenario.refused]
- [x] [serial] Sort and deduplicate subjects and dependencies for order-independent output. r[kiln.interchange.statement.scenario.canonical]

## Phase 2: Surface and verification

- [x] [serial] Retain statements under the provenance report kind and render subjects, dependencies and identities in the Evidence view with a non-claim. r[kiln.interchange.statement.scenario.evidence]
- [x] [serial] Add the golden-file, refusal, ordering and role-separation tests plus the UI asset syntax check. r[kiln.interchange.statement.scenario.canonical]
