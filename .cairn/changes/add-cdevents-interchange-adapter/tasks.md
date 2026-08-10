## Phase 1: Registry and profile

- [ ] [serial] Pin CDEvents, supported event and subject versions, CloudEvents bindings, licenses, algorithms, and non-claims. r[kiln.cdevents.profile] r[kiln.cdevents.nonclaims]
- [ ] [serial] Select the initial bounded event registry and explicit field mappings. r[kiln.cdevents.profile]
- [ ] [depends:cdevents-registry] Add typed Nickel version, event, field, loss, duplicate, correlation, and bound policy. r[kiln.cdevents.profile]

## Phase 2: Pure codecs and mappings

- [ ] [serial] Implement pure CloudEvents envelope admission and encoding. r[kiln.cdevents.envelope]
- [ ] [depends:cdevents-envelope] Implement pure per-type CDEvents import and export mappings. r[kiln.cdevents.mapping]
- [ ] [depends:cdevents-mapping] Implement duplicate, correlation, loss, and authority decisions. r[kiln.cdevents.import]
- [ ] [parallel] Add positive and negative pure-core fixtures. r[kiln.cdevents.conformance]

## Phase 3: Adapter crate

- [ ] [depends:cdevents-profile] Add `kiln-adapter-cdevents` without network or provider authority. r[kiln.cdevents.profile] r[kiln.cdevents.nonclaims]
- [ ] [depends:cdevents-import-core] Add bounded codec APIs and exact round-trip fixtures. r[kiln.cdevents.envelope] r[kiln.cdevents.mapping]
- [ ] [parallel] Add dependency gates that keep CDEvents types outside core crates. r[kiln.cdevents.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add canonical producer fixtures and an independent consumer. r[kiln.cdevents.conformance]
- [ ] [serial] Document supported events, loss policy, authority, and non-claims. r[kiln.cdevents.profile] r[kiln.cdevents.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.cdevents.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.cdevents.conformance] r[kiln.cdevents.nonclaims]
