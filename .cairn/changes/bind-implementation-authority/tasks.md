## Phase 1: Coverage contract

- [x] [serial] Add the closed protected-operation coverage map with its Basalt operation ids, owners, effect classes and candidate rule, plus positive and negative tests. r[kiln.implementation.authority.coverage]

## Phase 2: Cohort and import

- [ ] [serial] Upgrade `vendor/ucan` to the Basalt cohort `c483c7b58c42ec6636e9b8b8c0f73a2b609bf21e` and refresh its source record. r[kiln.implementation.authority.cohort]
- [ ] [serial] [depends:kiln.implementation.authority.cohort] Add the repo-owned Basalt import with its manifest rewrite, source record and license entry. r[kiln.implementation.authority.cohort.scenario.import]

## Phase 3: Authority port

- [ ] [serial] [depends:kiln.implementation.authority.coverage] Add the typed authority request/decision port in core. r[kiln.implementation.authority.binding]
- [ ] [serial] [depends:kiln.implementation.authority.cohort.scenario.import] Add the `kiln-adapter-basalt` crate with positive and negative fixtures over the packaged Basalt policy. r[kiln.implementation.authority.enforcement]

## Phase 4: Enforcement

- [ ] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for claim acquire/renew and dispatch at the implementer admission boundary. r[kiln.implementation.authority.enforcement.scenario.absent]
- [ ] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for candidate submission, verification admission and cancellation. r[kiln.implementation.authority.separation]
- [ ] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for publication, integration and lifecycle completion, keeping each grant separate. r[kiln.implementation.authority.separation.scenario.widen]
- [ ] [serial] [depends:kiln.implementation.authority.enforcement] Revalidate before each effect and block cancelled, superseded or revoked attempts. r[kiln.implementation.authority.currentness]

## Phase 5: Compatibility and evidence

- [ ] [serial] [depends:kiln.implementation.authority.enforcement] Add the explicit local-operator profile with recorded provenance and no role-only or fallback path. r[kiln.implementation.authority.compatibility]
- [ ] [serial] [depends:kiln.implementation.authority.currentness] Record bounded redacted decisions with principal, work/attempt/fence, policy, operation and denial reasons. r[kiln.implementation.authority.enforcement.scenario.replay]
- [ ] [serial] [depends:kiln.implementation.authority.compatibility] Update the operator guide, changelog, license map and run the focused, workspace and Cairn checks. r[kiln.implementation.authority.cohort]

## Blocked prerequisite

Phase 2 cannot proceed under the current pins. Kiln pins Rust 1.88.0 and Cargo
rejects the resolved graph on these declarations: `replay-ledger-core` and
`revocation-view-core` at `a0de7930` (1.96), `bounded-http` (1.96),
`detect-coding-agent` (1.91, via `secretspec 0.17.0` with default features off),
and Basalt's own optional durable pins. Two same-name same-version vendored UCAN
path packages also cannot coexist.

Measured on rustc 1.88.0: the two durable cores, both `bounded-http` crates,
`detect-coding-agent` 0.1.1 and `secretspec 0.17.0` without default features all
compile, so declaration alignment is technically feasible. The operator selected
that upstream alignment. Order: durable-authority-state cores, bounded-http,
the `detect-coding-agent` pin, UCAN's dependency bump, Basalt's re-vendor, then
this change. The rejected local UCAN upgrade was reverted byte-for-byte; the
workspace checks clean at `a125e597`.
