## Phase 1: Implementation

- [x] [serial] Add exact preview descriptor admission with build, candidate, runtime closure, engine, trust and finite bounds. r[kiln.preview.exact]
- [x] [serial] Implement bounded existing-store verification/export and isolated real TryNix consumer with separate preview observations. r[kiln.preview.runtime]
- [x] [serial] Add review-page action and positive/negative behavioral coverage; supply shared wiring and exact proof commands. r[kiln.preview.review]
- [x] [serial] Build only from an exact successful canonical Engine run/node/candidate/attempt through the bounded Nix adapter, retaining actual product/store observations on the existing run record before descriptor production. r[kiln.preview.exact]
- [x] [serial] Project only descriptors whose run is the review core's currently selected run and whose product, attempt, candidate and receipt match the retained canonical build record. r[kiln.preview.review]

- [x] [serial] Add an optional, bounded private local-store root to the native Nix provider/profile; bind it into operation identity and explicitly select it for every canonical build reader/writer while preserving default system-store behavior. Route fixture evaluation/signing and read-only preview production through that exact owned store; retain logical `/nix/store` identities. Add paired accepted/rejected profile coverage. r[kiln.preview.owned_store]

## Phase 2: Coordinated verification

- [x] [serial] Main agent runs coordinated service regressions and a real canonical CI build-to-retained-review-descriptor browser cohort; retains exact identities and any external prerequisite. r[kiln.preview.runtime]
