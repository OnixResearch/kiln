## Phase 1: Durable ownership

- [x] [serial] Preserve current claim, frozen work, aggregate budget and unknown-outcome barriers through restart. r[kiln.implementation.unknown_barrier]
- [x] [serial] Atomically apply admitted source supersession and refuse stale results/publication. r[kiln.implementation.current_authority]

## Phase 2: Consumer integration

- [x] [serial] Expose exact recovery and independent execution consumers through existing CLI stages. r[kiln.implementation.recovery_cli]
- [x] [serial] Add positive and negative crash/replay/cancel/stale behavioral fixtures and document actual qualification prerequisites. r[kiln.implementation.unknown_barrier.scenario.restart]
