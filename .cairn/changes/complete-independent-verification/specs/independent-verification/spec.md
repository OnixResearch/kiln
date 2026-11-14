## ADDED Requirements

### Requirement: Independently executed acceptance profile

r[kiln.implementation.verification] Kiln MUST execute an immutable admitted verification profile outside worker authority, independently resolve repository/base/candidate/tree and complete changed paths, retain actual command and artifact identities, cover every acceptance case, and use the same frozen bug reproducer on baseline and candidate. Nonpassing or unknown observations MUST NOT authorize completion. Cairn MUST reuse its authoritative completion evaluator and canonical CI; no-change MUST require explicit policy and independent passing evidence.

#### Scenario: Exact independent evidence passes

r[kiln.implementation.verification.scenario.independent]
- GIVEN exact admitted work and independently resolved candidate facts
- WHEN all frozen checks execute and required artifacts match
- THEN canonical typed checks MUST bind that exact candidate

#### Scenario: Worker success cannot substitute

r[kiln.implementation.verification.scenario.forged]
- GIVEN forged worker success, missing evidence, stale facts or changed protected requirements
- WHEN independent verification runs
- THEN completion MUST remain unmet
