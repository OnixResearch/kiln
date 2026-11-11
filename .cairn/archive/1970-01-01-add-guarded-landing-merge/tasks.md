## Phase 1: Landing

- [x] [serial] Re-admit the full observation and merge with the exact head SHA. r[kiln.github.landing.merge]
- [x] [serial] Read back the merged PR and base ref and bind the receipt. r[kiln.github.landing.merge.scenario.landed]

## Phase 2: Ambiguity

- [x] [serial] Reconcile refusal/timeout by read-back and keep unobservable merges unknown. r[kiln.github.landing.merge.scenario.refused]
- [x] [serial] Exclude issue/source-status mutation and atomic-base claims. r[kiln.github.landing.merge]
