## Phase 1: Handoff contract

- [x] [serial] Add the versioned handoff, evidence and acceptance-obligation digest. r[kiln.implementation.candidate_handoff]
- [x] [serial] Bind work/version/attempt/fence/base/repository and verification profile exactly. r[kiln.implementation.candidate_handoff.scenario.refused]
- [x] [serial] Refuse out-of-scope/duplicate paths, duplicate evidence, transferred acceptance and equal candidate/tree. r[kiln.implementation.candidate_handoff.scenario.refused]

## Phase 2: Retention and shell

- [x] [serial] Retain the handoff idempotently and move the attempt to candidate-produced. r[kiln.implementation.candidate_handoff.scenario.admit]
- [x] [serial] Add the operator handoff command and documentation. r[kiln.implementation.candidate_handoff]
