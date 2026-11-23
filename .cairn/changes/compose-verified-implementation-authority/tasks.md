## Phase 1: Contracts

- [x] [serial] Add the typed `kiln.implementation-authority-config.v1` Nickel contract with accepting and rejecting fixtures, its Rust loader with the same bounds, and `Implementer::select_authority`. r[kiln.implementation.authority.config]
- [x] [serial] Replace the payload digest with `payload_digest_v2` (incarnation, effect identity, expected target) at every call site. r[kiln.implementation.authority.payload]
- [x] [serial] Add the durable replay, revocation-view and decision tables in `<ledger>.authority` with `AuthorityStateStore`. r[kiln.implementation.authority.state]
- [x] [serial] [depends:kiln.implementation.authority.state] Add the role-separated holder signer and the signed-token test support. r[kiln.implementation.authority.selection]

## Phase 2: Composition

- [x] [serial] [depends:kiln.implementation.authority.config] Select the profile through one helper in the GitHub implementer, `lattice-dispatch`, the Lattice worker, the fleet store, the campaign drive and `verify-implementation`. r[kiln.implementation.authority.config]
- [x] [serial] [depends:kiln.implementation.authority.payload] Reconcile an observed dispatch as a fresh invocation over the original reservation effect. r[kiln.implementation.authority.reconcile]
- [x] [serial] Re-read the fence in the transport transaction before the no-policy return and add the two-connection interleaving tests. r[kiln.implementation.authority.interleaving]

## Phase 3: Coverage

- [x] [serial] Fill `ENTRY_POINTS` for every `Implementer` public function, CLI subcommand, fleet route and campaign control route, and add the source-scanning coverage test. r[kiln.implementation.authority.coverage.entry_points]
- [x] [serial] [depends:kiln.implementation.authority.coverage.entry_points] Admit `claim_next` and `claim_next_deferred` as `ClaimAcquire` inside the claim transaction, with a refusal test. r[kiln.implementation.authority.coverage.entry_points.scenario.claim]
- [x] [serial] [depends:kiln.implementation.authority.config] Add the end-to-end test that selects the verified profile from configuration files and drives claim, dispatch, renew and cancel through durable state. r[kiln.implementation.authority.state]
- [x] [serial] [depends:kiln.implementation.authority.coverage.entry_points] Admit `Dispatch` inside the transactions of `reserve_dispatch`, `reserve_dispatch_plan`, `start_execution` and the fleet session bind, recheck it before the fleet launch effects, and reclassify them and the CLI `reserve`, `reserve-plan` and `start` as `Protected(Dispatch)`. r[kiln.implementation.authority.dispatch]
- [x] [serial] [depends:kiln.implementation.authority.dispatch] Add the non-mutating `authorize_dispatch_plan` pre-check before the Lattice worker reads its credential, reusing its session in the reservation, with an injected-credential-source test. r[kiln.implementation.authority.dispatch.scenario.credential]
- [x] [serial] [depends:kiln.implementation.authority.dispatch] Test every dispatch step admitted under the verified profile and the local operator, and denied under the absent profile, a revoked grant, a stale revocation view and a wrong attempt or fence, committing and starting nothing. r[kiln.implementation.authority.dispatch.scenario.denied]
- [x] [serial] [depends:kiln.implementation.authority.config] Pass the campaign drive's own authority source to its worker, capture and verifier roles, and test that each role resolves the drive's profile, the verified worker reaches its transport check and the absent profile is refused before any credential read or reservation. r[kiln.implementation.authority.config.scenario.roles]
- [x] [serial] [depends:kiln.implementation.authority.selection] Load only the holder seeds of each entry point's own roles (the worker, capture role and fleet Coding, the verifier Verification, CLI subcommands from their rows, the drive per call site), refuse an operation of an unloaded role as holder-not-loaded before signing or mutation, and test the scoped roles with the other seeds absent or unreadable. r[kiln.implementation.authority.selection.scenario.scoped]

## Phase 4: Documentation and checks

- [x] [serial] Update the runbook authority section and remote-guarantee table, `implementation-work.md`, `http-observation.md` and the changelog. r[kiln.implementation.authority.config]
- [x] [serial] Run the workspace, wasm, Nickel, Octet and Cairn checks and record the evidence. r[kiln.implementation.authority.coverage.entry_points]

## Open decisions

- Native-Git requests and delivery still carry only the local-operator profile.
