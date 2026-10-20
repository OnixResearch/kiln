## Phase 1: Cohort and profile

- [x] [serial] Vendor and pin the exact UCAN cohort, including `ucan`, `ucan-core`, and `verified-logic`, and update the license map and reference pins. r[kiln.observe.http.ucan_profile]
- [x] [serial] Add typed Nickel `ucan_holder_invocation_v1` auth profile fields and positive/negative fixtures. r[kiln.observe.http.ucan_profile]
- [x] [parallel] Reject cohort drift, malformed issuer, unsupported operation profile, and missing bounds before serving. r[kiln.observe.http.ucan_profile.scenario.drift]

## Phase 2: Holder verification

- [x] [serial] Add the UCAN auth variant and bounded authorization-header parsing. r[kiln.observe.http.ucan_holder]
- [x] [depends:ucan-header] Verify the compact token, proof chain, revocation, capability, operation profile, request binding, time window, caveats, and holder signature. r[kiln.observe.http.ucan_holder]
- [x] [parallel] Return bounded authorization rejections for every failed check without projection. r[kiln.observe.http.ucan_holder.scenario.denied]

## Phase 3: Replay

- [x] [depends:ucan-holder] Implement bounded holder nonce replay admission after all non-consuming checks. r[kiln.observe.http.ucan_replay]
- [x] [parallel] Deny duplicate and over-bound nonces without projection. r[kiln.observe.http.ucan_replay.scenario.replay] r[kiln.observe.http.ucan_replay.scenario.bound]

## Phase 4: Conformance

- [x] [serial] Add positive fixtures for authorized holder serving, proof chains, and the unchanged Ed25519 path. r[kiln.observe.http.ucan_holder.scenario.authorized]
- [x] [serial] Add negative fixtures for missing/malformed tokens, proof and revocation failure, wrong audience/capability/request binding, expiry, caveats, replay, and cache exhaustion. r[kiln.observe.http.ucan_holder.scenario.denied]
- [x] [parallel] Add authority-promotion and non-claim fixtures. r[kiln.observe.http.ucan_nonclaims]

## Phase 5: Evidence

- [x] [serial] Document the UCAN cohort pin, header contract, capability mapping, proof/revocation policy, replay, caveat denial, and non-claims. r[kiln.observe.http.ucan_profile] r[kiln.observe.http.ucan_nonclaims]
- [x] [serial] Run formatting, tests, Clippy, Nickel, dependency, license, Cairn, Tracey, and Nix gates. r[kiln.observe.http.ucan_holder]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.observe.http.ucan_holder] r[kiln.observe.http.ucan_nonclaims]
