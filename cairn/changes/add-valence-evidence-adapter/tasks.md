## Phase 1: Contract pin

- [ ] [serial] Pin the Valence revision, licenses, Evidence IR schema, sidecar schema, role registry, span registry, and receipt contract. r[kiln.valence.profile] r[kiln.valence.nonclaims]
- [ ] [depends:valence-contract-pin] Add typed Nickel schema, role, span, limit, transport, and non-claim policy. r[kiln.valence.profile]
- [ ] [parallel] Add profile drift and unsupported-role fixtures. r[kiln.valence.conformance]

## Phase 2: Pure projection

- [ ] [serial] Implement pure subject, fact, identity, role, span, and non-claim projection. r[kiln.valence.projection]
- [ ] [depends:valence-projection] Implement pure import and round-trip admission without state authority. r[kiln.valence.import]
- [ ] [parallel] Add role-confusion, stale-subject, malformed-span, and lossy-mapping tests. r[kiln.valence.conformance]

## Phase 3: Adapter shell

- [ ] [depends:valence-profile] Add `kiln-adapter-valence` outside core dependency graphs. r[kiln.valence.profile] r[kiln.valence.nonclaims]
- [ ] [depends:valence-import-core] Implement bounded export, validation, receipt, and optional transport operations. r[kiln.valence.projection] r[kiln.valence.import]
- [ ] [parallel] Add runtime independence and provider-unavailable tests. r[kiln.valence.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add pinned producer and independent round-trip consumer fixtures. r[kiln.valence.conformance]
- [ ] [serial] Document roles, subjects, non-claims, transport, and drift behavior. r[kiln.valence.profile] r[kiln.valence.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.valence.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.valence.conformance] r[kiln.valence.nonclaims]
