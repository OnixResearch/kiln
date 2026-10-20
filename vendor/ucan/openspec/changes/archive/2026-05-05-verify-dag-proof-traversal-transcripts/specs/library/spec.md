## ADDED Requirements

### Requirement: Verified DAG Proof Traversal Transcript [r[ucan.verified-dag-proof-traversal-transcript]]

The library MUST expose a finite DAG proof traversal transcript bridge that validates caller-supplied entries, topologically ordered parent indices, budgets, capability attenuation, caveat preservation, leaf authorization, and revocation facts without performing token parsing, proof fetching, signature verification, clock reads, network I/O, filesystem I/O, or backend lookup.

#### Scenario: Multi-parent DAG accepts attenuated leaf [r[ucan.verified-dag-proof-traversal-transcript.scenario.accepts-attenuated-leaf]]

- GIVEN a caller-supplied DAG transcript whose parent indices point only to earlier entries
- AND every parent link narrows issuer/audience, time, capabilities, and caveats
- AND the leaf capability authorizes the requested target
- WHEN the bridge validates the transcript against caller-supplied budgets
- THEN the transcript is accepted.

#### Scenario: DAG rejects cycles and escalation [r[ucan.verified-dag-proof-traversal-transcript.scenario.rejects-cycles-escalation]]

- GIVEN a caller-supplied DAG transcript with a self/forward parent index or an edge that escalates authorization
- WHEN the bridge validates the transcript
- THEN the transcript is rejected without consulting external proof or revocation backends.
