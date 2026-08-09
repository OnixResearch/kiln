## Phase 1: Contract and profiles

- [ ] [serial] Pin GitHub webhook, API, event, status, check-run, pull-request, merge, and license documentation cohorts. r[kiln.github.profile] r[kiln.github.nonclaims]
- [ ] [depends:github-contract-pin] Add typed Nickel trigger, status, collaboration-read, and collaboration-mutation profiles. r[kiln.github.profile]
- [ ] [parallel] Add capability, scope, secret, repository, authority, rate, and bound fixtures. r[kiln.github.conformance]

## Phase 2: Pure mappings

- [ ] [serial] Implement pure webhook event, identity, signature-observation, and replay admission. r[kiln.github.trigger]
- [ ] [parallel] Implement pure status planning and observation admission. r[kiln.github.status]
- [ ] [parallel] Implement pure collaboration mapping and fresh merge preflight. r[kiln.github.collaboration] r[kiln.github.merge]
- [ ] [parallel] Add positive and negative mapping tests. r[kiln.github.conformance]

## Phase 3: Shell capabilities

- [ ] [depends:github-profile] Add `kiln-adapter-github` outside core dependency graphs. r[kiln.github.profile] r[kiln.github.nonclaims]
- [ ] [depends:github-trigger-core] Implement bounded webhook intake with HMAC-SHA256 and durable replay protection. r[kiln.github.trigger]
- [ ] [depends:github-status-core] Implement status dispatch, lookup, rate handling, and reconciliation. r[kiln.github.status]
- [ ] [depends:github-collaboration-core] Implement bounded collaboration reads. r[kiln.github.collaboration]
- [ ] [depends:github-merge-core] Implement merge only after trigger, status, and read conformance pass. r[kiln.github.merge]

## Phase 4: Evidence

- [ ] [serial] Add pinned payload and API fixtures with independent consumers. r[kiln.github.conformance]
- [ ] [serial] Document credentials, authority, rate behavior, recovery, and non-claims. r[kiln.github.profile] r[kiln.github.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.github.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.github.conformance] r[kiln.github.nonclaims]
