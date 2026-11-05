## Phase 1: Shared contracts

- [x] [serial] Create the `kiln-nix-store` facts crate and workspace wiring. r[kiln.nix_store.facts]
- [x] [serial] Add bounded derivation admission, NAR/reference facts, input closure and narinfo projection. r[kiln.nix_store.derivation] r[kiln.nix_store.nar] r[kiln.nix_store.cache]
- [x] [serial] Extend core build types with constraints, store objects and input facts while preserving absent-field bytes. r[kiln.nix_store.contracts]

## Phase 2: Providers

- [x] [serial] Parse and validate the exact `.drv` in the Nix adapter and derive expected outputs. r[kiln.nix_store.derivation]
- [x] [serial] Use admitted derivation outputs in `kiln-nix-build` composition. r[kiln.nix_store.derivation]
- [x] [serial] Record NAR/store facts in the Nix and Mantle adapters. r[kiln.nix_store.nar] r[kiln.nix_store.contracts]
- [x] [serial] Add the read-only cache projection command. r[kiln.nix_store.cache]

## Phase 3: Verification

- [x] [serial] Add positive and negative fixtures and focused tests. r[kiln.nix_store.derivation] r[kiln.nix_store.nar] r[kiln.nix_store.cache] r[kiln.nix_store.contracts]
- [x] [serial] Run focused/full Cargo, Clippy, rustfmt, Cairn validation and record evidence. r[kiln.nix_store.derivation] r[kiln.nix_store.nar]
