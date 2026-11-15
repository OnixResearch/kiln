## ADDED Requirements

### Requirement: Exact preview evidence comparison
r[kiln.preview.comparison]
The preview server MUST retain only bounded browser reports whose identity, binding, engine and invocation are copied from the admitted descriptor, under a domain-tagged BLAKE3 identity with finite expiry and a bounded count. The review service MUST project only verified, unexpired records for the change's project, repository and revision commits, and MUST mark a record current only for the current revision commit and the selected successful CI run. A comparison MUST pair one projected baseline with one current candidate, derive a deterministic identity from both and the rule version, and report every engine, platform, argument or executable mismatch. Unknown, expired or unprojected subjects MUST be refused without substitution. Only the current candidate MAY offer a launch. Observations MUST NOT change CI, review, merge or implementation-completion state.

#### Scenario: Reviewer compares a baseline with the current candidate
- GIVEN retained observations for a baseline and the current candidate
- WHEN the reviewer selects both or reloads the deep link
- THEN both exact identities, phases and bounded outputs appear side by side with the comparison identity and any incompatibility reasons

#### Scenario: Forged or mismatched evidence is refused
- GIVEN a report for another descriptor, a foreign origin, an oversized or malformed body, an unknown phase or a full store
- WHEN it is submitted or read back
- THEN no record is retained or projected and the refusal is explicit

#### Scenario: Candidate moves
- GIVEN a retained comparison whose candidate revision is no longer current
- WHEN the review is reloaded or a late response arrives
- THEN the observation is shown as historical, cannot be compared as the current candidate and offers no launch
