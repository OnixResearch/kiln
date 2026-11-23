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

## Phase 4: Documentation and checks

- [x] [serial] Update the runbook authority section and remote-guarantee table, `implementation-work.md`, `http-observation.md` and the changelog. r[kiln.implementation.authority.config]
- [x] [serial] Run the workspace, wasm, Nickel, Octet and Cairn checks and record the evidence. r[kiln.implementation.authority.coverage.entry_points]

## Open decisions

- The fleet tick launches a worker session after `fleet_bind_session` with no `Dispatch` admission (recorded as a gap in `ENTRY_POINTS`).
- `reserve_dispatch` and `reserve_dispatch_plan` are classified as durable intents; the GitHub CLI `reserve`/`start` path records no `Dispatch` admission.
- Native-Git requests and delivery still carry only the local-operator profile.
