## Phase 1: Durable host core and adapters

- [x] [serial] Define the durable hosted-operation states, transitions, invariants, and negative corruption cases. r[kiln.aspen_host.production_durability]
- [x] [serial] Add a durable operation ledger that reserves ingress before effects and binds terminal or uncertain outcomes atomically. r[kiln.aspen_host.production_durability]
- [x] [serial] Compose Aspen's durable native journal and a capability-rooted durable callback value adapter. r[kiln.aspen_host.production_durability]
- [x] [serial] Restore the native service, trigger links, acknowledgements, uncertainty, and logical sequence after restart. r[kiln.aspen_host.production_recovery]
- [x] [serial] Reconcile pending and uncertain Lattice operations without provider redispatch. r[kiln.aspen_host.production_recovery]
- [x] [serial] Add positive restart/replay tests and negative missing, changed, truncated, and corrupted-state tests. r[kiln.aspen_host.production_recovery]

## Phase 2: Bounded production provider

- [x] [serial] Define a typed Nickel profile for one exact Radicle repository, fixed Nix wrapper, report root, user, and resource bounds. r[kiln.radicle.hosted_nix_provider]
- [x] [serial] Add the bounded `kiln-radicle-nix-provider` shell with explicit source arguments and cleared child environment. r[kiln.radicle.hosted_nix_provider]
- [x] [serial] Publish deterministic bounded log and JSON reports for success and failure. r[kiln.radicle.hosted_nix_provider]
- [x] [serial] Reject wrong users, repositories, revisions, paths, commands, output floods, timeouts, and malformed profiles before false success. r[kiln.radicle.hosted_nix_provider]

## Phase 3: Production evidence and publication

- [x] [serial] Run all-target/all-feature tests, strict Clippy, Octet deny-all, and Nix package checks. r[kiln.aspen_host.production_evidence]
- [x] [serial] Run real-process restart, corruption, provider teardown, bounded load, backup/restore, and rollback fixtures. r[kiln.aspen_host.production_evidence]
- [x] [serial] Record exact receipts and preserve all durability, workflow, provider, host, and release non-claims. r[kiln.aspen_host.production_evidence]
- [ ] [serial] Publish the immutable root provider revision, update the nested host lock, and publish the final host cohort. r[kiln.aspen_host.production_evidence]
- [ ] [serial] Sync and archive only after every production gate passes. r[kiln.aspen_host.production_evidence]
