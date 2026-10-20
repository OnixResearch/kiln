## ADDED Requirements

### Requirement: Live DAG proof traversal transcripts

The library MUST route live multi-parent proof traversal transcripts through the verified DAG traversal bridge when recursive proof verification successfully resolves a representable finite proof DAG.

#### Scenario: Multi-parent proof verification emits checked DAG transcript

- GIVEN a compact UCAN whose direct proofs verify successfully through caller-supplied key resolver and proof store facts
- AND the resolved proof facts form a finite topologically ordered proof DAG representable by the live transcript model
- WHEN the shell returns a verified token
- THEN the emitted proof transcript MUST have been accepted by `proof_traversal_dag_transcript_is_valid`
- AND the transcript MUST preserve explicit budgets, visited proof references, parent indices, temporal validity, non-revocation facts, and capability attenuation facts.

#### Scenario: Runtime trust boundaries remain external

- GIVEN the shell emits and checks a live proof-DAG transcript
- WHEN documentation describes the verified boundary
- THEN it MUST state that compact decoding, JSON canonicalization, signature cryptography, proof-reference hashing, proof-store/resolver/revocation backend truth or freshness, and arbitrary caveat policies remain outside formal verification.
