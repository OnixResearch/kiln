## Phase 1: Boundary inventory

- [x] [serial] Inventory public provider contracts, callers, implementations, core decisions, and compatibility surfaces in existing adapter crates. r[kiln.providers.port_ownership]
- [x] [parallel] Record current positive and negative behavior for Jujutsu, Radicle broker, COB, and remote paths before migration. r[kiln.providers.boundary_conformance]

## Phase 2: Application contracts and pure decisions

- [x] [serial] Add narrow application-owned request, observation, and failure types for the migrated provider capabilities. r[kiln.providers.port_ownership] r[kiln.providers.error_ownership]
- [x] [depends:provider-contracts] Move Jujutsu ambiguity, COB freshness, remote admission, and outcome classification into value-based core functions. r[kiln.providers.policy_effect_split]
- [x] [parallel] Add positive and negative direct core tests without provider implementations. r[kiln.providers.policy_effect_split] r[kiln.providers.boundary_conformance]

## Phase 3: Adapter and shell migration

- [x] [depends:pure-provider-decisions] Make Jujutsu, Radicle broker, COB, and remote adapters implement the application-owned contracts. r[kiln.providers.port_ownership]
- [x] [depends:provider-contracts] Add typed infrastructure errors and explicit shell translation into domain, retry, cancellation, timeout, and unknown classes. r[kiln.providers.error_ownership]
- [x] [depends:adapter-port-implementations] Select concrete adapters only at visible composition roots. r[kiln.providers.composition_root]
- [x] [serial] Preserve supported public behavior through explicit compatibility adapters, then remove competing public adapter-owned ports. r[kiln.providers.port_ownership]

## Phase 4: Conformance and lifecycle

- [x] [parallel] Add shell-order tests that prove denial blocks effects and ambiguous calls cannot become success or not-started. r[kiln.providers.boundary_conformance]
- [x] [parallel] Add dependency and source-shape guards for port ownership, provider isolation, and composition-root selection. r[kiln.providers.port_ownership] r[kiln.providers.composition_root]
- [x] [serial] Run focused tests, workspace tests, Clippy, `no_std`, Nickel, Cairn, Tracey, and Nix gates. r[kiln.providers.boundary_conformance]
- [x] [serial] Record evidence, sync accepted specs, archive the change, and rerun validation. r[kiln.providers.boundary_conformance]
