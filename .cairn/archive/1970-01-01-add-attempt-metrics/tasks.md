## Phase 1: Receipt contract

- [x] [serial] Add typed absent/observed usage and the versioned metrics receipt. r[kiln.implementation.metrics]
- [x] [serial] Bind attempt/fence/work and claim/terminal timestamps; preserve additive state semantics. r[kiln.implementation.metrics.scenario.terminal]
- [x] [serial] Keep observation pure and default-off. r[kiln.implementation.metrics.scenario.default_off]
- [x] [serial] Refuse stale fences, malformed traces and pre-claim observations. r[kiln.implementation.metrics.scenario.binding]

## Phase 2: Retention and shell

- [x] [serial] Retain only the first explicitly recorded receipt. r[kiln.implementation.metrics.scenario.retained]
- [x] [serial] Add the operator `metrics` command with optional usage input. r[kiln.implementation.metrics.scenario.open]
