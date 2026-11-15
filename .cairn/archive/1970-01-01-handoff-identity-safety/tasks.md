## Phase 1: Pure contract

- [x] [serial] Persist originating session identity at begin and require exact identity/placement matching for every transition, reconciliation and serialized reopen. r[kiln.hosted.handoff.order.scenario.complete] r[kiln.hosted.handoff.order.scenario.order]
- [x] [serial] Stop rewriting checkpoint expectations; compare independent identity/source/target/operation facts and pass expectations unchanged to direct manifest admission. Refusals leave phase and manifest unchanged. r[kiln.hosted.handoff.manifest.scenario.disk]
- [x] [serial] Bind target admission and resume to exact operation, originating identity, admitted manifest, placements, target owner and cutover/fencing facts; reject stale/forged evidence atomically. r[kiln.hosted.handoff.order.scenario.order]

## Phase 2: Regressions and contract documentation

- [x] [serial] Add healthy complete-handoff and caller-B-against-A tests at each phase and after serialization/reopen; cover work/revision/attempt/fence/execution/session/placement/generation and crossed operation substitutions. r[kiln.hosted.handoff.order.scenario.complete] r[kiln.hosted.handoff.order.scenario.order]
- [x] [serial] Test independent manifest-expectation mismatches both directly and through handoff, plus stale/forged target admission and unchanged state on refusal. r[kiln.hosted.handoff.manifest.scenario.disk] r[kiln.hosted.handoff.order.scenario.order]
- [x] [serial] Update only the paused-handoff section of retained-session documentation to explain origin binding, expectation comparison, serialized compatibility and provider/non-claim boundary. r[kiln.hosted.handoff.order.scenario.complete]
- [x] [serial] Run applicable Cairn and focused verification gates, report exact commands/results and unrun native qualification; no live VMM or provider effects. r[kiln.hosted.handoff.order.scenario.complete]
