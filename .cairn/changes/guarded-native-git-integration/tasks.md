# Tasks

## Phase 1: Versioned native delivery core

- [x] [serial] Add independent native Git plan, pending-operation, observation, receipt and admission types without changing PR v1 serialization. r[kiln.implementation.native_git]
- [x] [serial] Bind exact source authority, work/version/attempt/fence, candidate/tree, remote/target/expected-old, current review/check/policy, provider and operation lineage. r[kiln.implementation.native_git]
- [x] [serial] Model target advancement as new candidate requiring new verification and review; reject stale authority, policy, fence and mismatched identities. r[kiln.implementation.native_git]

## Phase 2: Durable route and transport

- [x] [serial] Persist native branch and target intent before effects; restart uses read-only exact reconciliation, never replay dispatch. r[kiln.implementation.native_git]
- [x] [serial] Implement bounded supported Git remote branch publication and guarded non-force fast-forward; reject unsupported transport and explicit remote protection failures. r[kiln.implementation.native_git]
- [x] [serial] Integrate exact native receipt into durable implementation and Cairn lifecycle result without PR identity; preserve v1 PR consumers. r[kiln.implementation.native_git]

## Phase 3: Regression and operator qualification

- [x] [serial] Cover restart, both lost acknowledgements, target/branch race, non-FF, wrong remote/candidate, stale policy/review/fence, protection failure and receipt consumer. r[kiln.implementation.native_git]
- [x] [serial] Add reproducible opt-in real remote qualification entrypoint and focused native Git operator instructions with explicit no-live-qualification boundary. r[kiln.implementation.native_git]
- [x] [serial] Run proposal, design and task gates and record results before implementation. r[kiln.implementation.native_git]
