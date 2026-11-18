# Artifact preview delta

## ADDED Requirements

### Requirement: Opt-in structural preview evidence

r[kiln.preview.structural] A preview comparison MUST add structural source evidence only when the request explicitly selects it; without that selection the comparison response, launch rules and graphical cohort MUST be unchanged. The service MUST compute the evidence with the code review's structural function for the exact commit comparison of the baseline and candidate revisions, with exact whitespace and the review's default bounds, and no grammar MUST run in the browser. The record MUST bind the comparison identity, both revision commits, the structural rules, a grammar identity, the bounds and either each file's structural rows or its named fallback, under a domain-tagged BLAKE3 identity. The grammar identity MUST cover the exact package, version and registry checksum of the Tree-sitter runtime, highlight engine and every grammar, checked against the workspace lock file. The record is display-only preview evidence: it MUST NOT change CI, review approval, merge or implementation-completion state.

#### Scenario: Off unless selected

r[kiln.preview.structural.scenario.off]
- GIVEN a baseline and a current candidate observation
- WHEN the comparison is requested without selecting structural evidence
- THEN the response MUST be the existing comparison with no structural record, and selecting it MUST add only the record

#### Scenario: Frozen fixture matches the structural review

r[kiln.preview.structural.scenario.fidelity]
- GIVEN a frozen fixture of a pure reformat, a moved unchanged function, one edit and named fallbacks, retained with its comparison settings
- WHEN structural preview evidence and the review's structural mode compare the same two commits
- THEN each file MUST carry the same structural rows or the same named fallback on both sides, the reformat MUST show no change and the function MUST be reported as moved

#### Scenario: Unusable input leaves an explicit reason

r[kiln.preview.structural.scenario.fallback]
- GIVEN incompatible subjects, a file without a grammar, binary or truncated content, a parse error, a hit bound, an unreadable source comparison or an oversized record
- WHEN structural evidence is requested
- THEN the record MUST carry the named incompatibility, fallback or unavailability reason, MUST retain no partial tree and MUST NOT substitute another observation or revision
