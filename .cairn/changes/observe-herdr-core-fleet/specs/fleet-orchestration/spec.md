## ADDED Requirements

### Requirement: Herdr Core observation isolation

r[kiln.fleet.herdr_core_observation]
Fleet MUST admit Core observations through an explicitly selected versioned local endpoint independently of dispatch, and MUST NOT expose terminal content or infer assignment from Core status or display data.

#### Scenario: A remote line drops
- GIVEN a mapped Core machine with a previously observed terminal
- WHEN the Core connection or its remote line no longer provides a fresh observation
- THEN Fleet MUST distinguish stale or unobserved data from zero live terminals, and retain independent Cairn and issue reads
