## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST admit decoded proof-reference collections only when finite collection facts, including non-empty references, traversal count bounds, and the selected duplicate policy, are verified before proof traversal continues.

#### Scenario: Decoded proof-reference collection uses verified bridge [r[library.compact-token.scenario.proof-reference-collection-admission]]

- GIVEN decoded proof-reference byte entries produced by the shell
- WHEN the shell prepares proof traversal inputs
- THEN it MUST route non-empty-entry, count-bound, and duplicate-policy facts through a vendored verified collection bridge before traversal
- AND collection-shape rejection MUST fail closed before proof-store lookup

#### Scenario: Proof-reference collection admission keeps backend semantics outside the verified claim [r[library.compact-token.scenario.proof-reference-collection-boundary-limits]]

- GIVEN decoded proof-reference collection facts admitted by the verified predicate
- WHEN the shell later resolves proofs or checks revocation
- THEN the library MUST continue to document proof-reference semantic truth, hash derivation correctness, proof-store truth/freshness/provenance, revocation truth/freshness, network/storage behavior, and cryptography as outside the collection-admission claim
