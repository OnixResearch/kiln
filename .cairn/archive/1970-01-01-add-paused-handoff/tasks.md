## Phase 1: Manifest

- [x] [serial] Bind session/work/attempt/fence/execution, placement, profile, disk generation and distinct objects. r[kiln.hosted.handoff.manifest]
- [x] [serial] Refuse old memory with a different disk identity or generation. r[kiln.hosted.handoff.manifest.scenario.disk]

## Phase 2: Ordering

- [x] [serial] Order intent, quiescence, source pause, verification, target admission, resume and retirement. r[kiln.hosted.handoff.order.scenario.complete]
- [x] [serial] Refuse stale/out-of-order owners and commands. r[kiln.hosted.handoff.order.scenario.order]
- [x] [serial] Keep uncertainty reconcilable without retirement or capacity release. r[kiln.hosted.handoff.manifest.scenario.reconcile]
