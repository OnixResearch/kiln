## Phase 1: Authenticated intake

- [x] [serial] Add bounded authenticated `issues` delivery admission reusing the existing HMAC and header validation. r[kiln.implementation.issue_intake]
- [x] [serial] Require enrolled repository and actor and derive the sequence from the strict issue update timestamp. r[kiln.implementation.issue_intake.scenario.admit]

## Phase 2: Normalization and refusal

- [x] [serial] Build one work record from operator policy plus exact event bytes; closed issues produce a closed observation. r[kiln.implementation.issue_intake.scenario.lifecycle]
- [x] [serial] Refuse forged signatures, mismatched repositories, unenrolled actors and malformed payloads. r[kiln.implementation.issue_intake.scenario.refused]
- [x] [serial] Ignore unsupported actions and unrelated events without creating work. r[kiln.implementation.issue_intake]
