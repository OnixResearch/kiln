# Change Review Specification

## ADDED Requirements

### Requirement: Optional revision-bound session trace identities

r[kiln.review.session_traces] Kiln MAY retain operator-enabled session trace
pointer identities on an exact review revision. The feature MUST default off;
missing observations MUST remain absent, incompatible observations MUST be
explained, and no trace MUST confer verification, ownership, approval or merge
authority. Raw session bodies and credentials MUST NOT enter journal metadata.

#### Scenario: Exact admitted notes

r[kiln.review.session_traces.scenario.notes]
- GIVEN locally admitted exact commits with present, absent or malformed trace notes
- WHEN the optional source is enabled
- THEN documented IDs MUST bind to their exact commit and Kiln content identities
- AND missing notes MUST differ from unsupported or incompatible observations
- AND transferred commit/digest bindings MUST be refused

#### Scenario: Disabled and durable replay

r[kiln.review.session_traces.scenario.replay]
- GIVEN old journals or disabled trace capture
- WHEN replayed or new revisions are submitted
- THEN existing serialized revision bytes MUST remain unchanged
- AND enabled captured observations MUST survive restart without being re-read or rewritten

#### Scenario: Separate untrusted review context

r[kiln.review.session_traces.scenario.surface]
- GIVEN a selected revision with declared authorship, attested submission and trace pointers
- WHEN rendered
- THEN each MUST be separately labeled
- AND trace pointers MUST be bounded untrusted historical context with no session body or remote fetch
- AND the disabled review surface MUST remain unchanged

#### Scenario: No authority from traces

r[kiln.review.session_traces.scenario.authority]
- GIVEN equivalent candidates with and without valid trace observations
- WHEN checklist, CI, ownership, approval and submit decisions are evaluated
- THEN trace presence MUST NOT change those decisions
- AND malformed or stale trace metadata MUST NOT manufacture acceptance
