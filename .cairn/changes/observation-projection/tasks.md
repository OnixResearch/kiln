## Phase 1: Profile and view vocabulary

- [ ] [serial] Define the bounded observation profile: supported view kinds, named bounds, paging policy, cursor windows, and redaction policy. r[kiln.observe.profile]
- [ ] [serial] Add typed Nickel observation profiles with positive and negative profile fixtures. r[kiln.observe.profile]
- [ ] [parallel] Add view-vocabulary admission: unknown kinds fail before projection. r[kiln.observe.profile]

## Phase 2: Pure projection core

- [ ] [serial] Add `kiln-observe-core` as a pure `no_std + alloc` crate with no new external dependency. r[kiln.observe.projection]
- [ ] [depends:observe-profile] Implement deterministic read-only projection over admitted state, profile, and window facts. r[kiln.observe.projection]
- [ ] [parallel] Implement `kiln.observe.view.v1` view identity binding over canonical bytes, state revision, request, and window. r[kiln.observe.identity]

## Phase 3: Bounded admission and redaction

- [ ] [depends:observe-projection-core] Implement page, window, attribute, and value bound admission with visible truncation facts. r[kiln.observe.safety]
- [ ] [depends:observe-safety-core] Implement unknown-target and unknown-state handling and redaction before encoding. r[kiln.observe.safety] r[kiln.observe.identity]
- [ ] [parallel] Admit event-tail and log-range windows as exact shell-supplied observations. r[kiln.observe.safety] r[kiln.observe.identity]

## Phase 4: Conformance fixtures

- [ ] [serial] Add deterministic positive fixtures for every supported view, rebuild, and identity parity. r[kiln.observe.conformance]
- [ ] [serial] Add negative fixtures for unknown views, unknown states, unknown targets, page overflow, out-of-window cursors, redaction, and read-only violation. r[kiln.observe.conformance]
- [ ] [parallel] Add authority-promotion and non-claim fixtures. r[kiln.observe.nonclaims]

## Phase 5: Evidence

- [ ] [serial] Document the view vocabulary, bounds, identity, redaction, and non-claims. r[kiln.observe.profile] r[kiln.observe.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.observe.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.observe.conformance] r[kiln.observe.nonclaims]
