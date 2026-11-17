## Phase 1: Continuation core

- [x] [serial] Add `kiln_core::implementation::continuation`: wake hints and coalescing, policy validation, the pure decision, outcome application and capped backoff, with positive and negative cases. r[kiln.implementation.campaign.continuation] r[kiln.implementation.campaign.continuation.scenario.coalesce] r[kiln.implementation.campaign.continuation.scenario.idle] r[kiln.implementation.campaign.continuation.scenario.pause] r[kiln.implementation.campaign.continuation.scenario.backoff]

## Phase 2: Durable record and loop

- [x] [serial] Persist the continuation record beside the campaign in immediate merge transactions, make a canonical capacity refusal a typed wait in the drive loop and exact admission, classify driver batches, and add `continue_bounded`. r[kiln.implementation.campaign.continuation.scenario.restart] r[kiln.implementation.campaign.continuation.scenario.rescan]
- [x] [serial] Record wake hints from decision resolution and admission continuation, and add scripted-owner cases for review resumption, lifecycle-only continuation, coalescing, mid-batch hints, stale versions, pause, refusal, backoff and restart. r[kiln.implementation.campaign.continuation.scenario.resume] r[kiln.implementation.campaign.continuation.scenario.lifecycle] r[kiln.implementation.campaign.continuation.scenario.coalesce] r[kiln.implementation.campaign.continuation.scenario.pause] r[kiln.implementation.campaign.continuation.scenario.restart]

## Phase 3: Native commands and verification

- [x] [serial] Add the `continue-implementation-campaign` and `wake-implementation-campaign` commands, the optional profile policy and its Nickel contract, and route the native driver through the typed refusal. r[kiln.implementation.campaign.continuation]
- [x] [serial] Run focused, workspace and Nix gates; update the runbook and changelog; register the active delta root; and pass Cairn gates, validation and traceability. r[kiln.implementation.campaign.continuation]
