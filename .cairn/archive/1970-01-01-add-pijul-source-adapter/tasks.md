## Phase 1: Contract and compatibility pin

- [x] [serial] Record the exact Pijul and `libpijul` release cohort, source revision, repository format, licenses, supported platforms, and native state semantics. r[kiln.source.pijul_configuration] r[kiln.source.pijul_state_identity]
- [x] [depends:pijul-contract-pin] Add a typed Nickel profile for versions, capabilities, bounds, entry kinds, compatibility facts, and non-claims. r[kiln.source.pijul_configuration] r[kiln.source.pijul_capabilities]
- [x] [parallel] Add positive profile fixtures and negative missing-field, zero-bound, unsupported-capability, wrong-version, and format-drift fixtures. r[kiln.source.pijul_configuration] r[kiln.source.pijul_conformance]

## Phase 2: Pure source core

- [x] [serial] Add bounded Pijul repository, channel, state, change-frontier, format-cohort, and observation facts to the source model. r[kiln.source.pijul_state_identity]
- [x] [depends:pijul-core-facts] Refactor provider-specific source facts into one tagged value when necessary, then update Jujutsu mappings and tests. r[kiln.source.pijul_state_identity]
- [x] [depends:pijul-core-facts] Implement pure Pijul fact validation for namespaces, required identities, duplicates, bounds, and mixed-provider states. r[kiln.source.pijul_state_identity]
- [x] [depends:pijul-core-facts] Implement pure snapshot-entry admission, deterministic ordering, explicit framing, and canonical manifest construction. r[kiln.source.pijul_snapshot]
- [x] [parallel] Add positive and negative core tests for exact states, changed states, malformed facts, unsafe paths, duplicates, unsupported entries, and exceeded bounds. r[kiln.source.pijul_state_identity] r[kiln.source.pijul_snapshot]
- [x] [parallel] Extend dependency gates to reject Pijul provider crates, repository handles, filesystem access, and process access from core crates. r[kiln.source.pijul_capabilities] r[kiln.source.pijul_nonclaims]

## Phase 3: Optional Pijul adapter shell

- [x] [depends:pijul-contract-pin] Add `kiln-adapter-pijul` to the workspace without adding it to core or standalone dependency graphs. r[kiln.source.pijul_adapter] r[kiln.source.pijul_nonclaims]
- [x] [depends:pijul-core-facts] Define the narrow repository port for discovery, exact-state resolution, and snapshot acquisition. r[kiln.source.pijul_adapter] r[kiln.source.pijul_capabilities]
- [x] [depends:pijul-repository-port] Implement the concrete read-only local repository shell with the pinned `libpijul` cohort. r[kiln.source.pijul_adapter]
- [x] [depends:pijul-local-shell] Use one consistent read view, or reject state movement across the resolution and acquisition boundary. r[kiln.source.pijul_state_identity] r[kiln.source.pijul_snapshot]
- [x] [depends:pijul-local-shell] Add named repository, change, entry, path, file, and total-byte bounds before unbounded collection or allocation. r[kiln.source.pijul_configuration] r[kiln.source.pijul_snapshot]
- [x] [depends:pijul-local-shell] Map provider failures into stable missing, damaged, unsupported, busy, changed, bounded, and provider-error diagnostics. r[kiln.source.pijul_adapter] r[kiln.source.pijul_conformance]
- [x] [parallel] Prove that the Pijul profile cannot request credentials, collaboration, triggers, status publication, or source mutation. r[kiln.source.pijul_capabilities] r[kiln.source.pijul_nonclaims]

## Phase 4: Repository fixtures and adversarial tests

- [x] [depends:pijul-local-shell] Add positive local repositories for one change, dependent changes, separate channels, equal trees with different histories, and repeatable acquisition. r[kiln.source.pijul_conformance]
- [x] [depends:pijul-local-shell] Add negative local repositories or bounded fault fixtures for missing channels, missing changes, damaged state, and unsupported formats. r[kiln.source.pijul_conformance]
- [x] [depends:pijul-snapshot-core] Add unsafe-path, duplicate-path, unsupported-entry, oversized-file, oversized-history, and total-bound rejection fixtures. r[kiln.source.pijul_snapshot] r[kiln.source.pijul_conformance]
- [x] [depends:pijul-consistent-read] Add a deterministic changed-state fixture that rejects mixed state and snapshot facts. r[kiln.source.pijul_state_identity] r[kiln.source.pijul_conformance]
- [x] [parallel] Add runtime-independence checks for core, standalone, source archives, and default CLI builds without Pijul runtime software. r[kiln.source.pijul_nonclaims]

## Phase 5: Documentation and completion evidence

- [x] [serial] Document Pijul profile setup, supported repository cohort, local read behavior, bounds, diagnostics, and non-claims. r[kiln.source.pijul_adapter] r[kiln.source.pijul_configuration] r[kiln.source.pijul_nonclaims]
- [x] [serial] Update the adapter inventory so Pijul appears as implemented only after the concrete shell and conformance gate pass. r[kiln.source.pijul_adapter] r[kiln.source.pijul_conformance]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std`, Nickel, dependency, fixture, Cairn, traceability, and focused Nix gates. r[kiln.source.pijul_conformance]
- [x] [serial] Record deterministic evidence for exact-state binding, canonical identity, failure behavior, compatibility pins, and runtime independence. r[kiln.source.pijul_conformance] r[kiln.source.pijul_nonclaims]
- [x] [serial] Sync the accepted source specification, confirm complete requirement coverage, archive the change, and rerun post-archive validation. r[kiln.source.pijul_adapter] r[kiln.source.pijul_nonclaims]
