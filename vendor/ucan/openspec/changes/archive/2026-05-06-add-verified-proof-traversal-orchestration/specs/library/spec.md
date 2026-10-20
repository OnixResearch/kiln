## ADDED Requirements

### Requirement: Verified proof traversal orchestration [r[library.verified-proof-traversal-orchestration]]
The library MUST route proof traversal summaries through vendored verified-logic predicates for structural budgets, cycle rejection, proof-link shape, and effective-delegation provenance before accepting delegated authorization.

#### Scenario: Traversal budget facts are verified [r[library.verified-proof-traversal-orchestration.scenario.budgets]]
- GIVEN a proof-chain verification request with configured max depth, max proof count, and compact-token byte limits
- WHEN the shell traverses proof references supplied by the proof store
- THEN it MUST route depth/count/input-size admission facts through verified logic before accepting the traversal
- AND traversal MUST fail closed when a configured budget is exceeded

#### Scenario: Cycle and duplicate proof references are rejected [r[library.verified-proof-traversal-orchestration.scenario.cycles]]
- GIVEN a proof traversal transcript containing visited proof references
- WHEN the shell detects a cycle or duplicate traversal edge that would revisit a proof reference
- THEN the shell MUST route the visited-reference facts through a verified structural predicate
- AND the traversal MUST be denied rather than reporting an effective delegation

#### Scenario: Effective delegation comes only from accepted chain [r[library.verified-proof-traversal-orchestration.scenario.effective-delegation-provenance]]
- GIVEN a candidate effective delegation produced after parent proof traversal
- WHEN the shell reports the delegated capability as authorized
- THEN the shell MUST verify that the candidate was derived from an accepted traversal transcript whose links, temporal narrowing, and capability attenuation facts were admitted
- AND a failed traversal stage MUST prevent effective delegation from being reported

#### Scenario: Traversal verification keeps backend truth outside the claim [r[library.verified-proof-traversal-orchestration.scenario.boundary-limits]]
- GIVEN a proof traversal transcript admitted by verified logic
- WHEN a consumer relies on proof bytes, proof-store responses, token parser output, signature checks, revocation freshness, or backend provenance
- THEN the library MUST continue to document those facts as runtime or caller-owned boundaries outside the traversal-orchestration claim
