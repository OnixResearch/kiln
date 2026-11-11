# Implementation Work Specification

## ADDED Requirements

### Requirement: Authenticated GitHub issue intake

r[kiln.implementation.issue_intake] Kiln MUST admit implementation intent from a
GitHub `issues` webhook only after bounding the body, validating the delivery and
event headers, and verifying the exact original bytes with the configured
HMAC-SHA256 secret. The repository and actor MUST match the reviewed policy and
enrollment. Acceptance obligations, requested outcome, permitted paths and
effects, verification profile, budget, terminal policy and priority MUST come
from operator policy, never from issue text. An opened, reopened or edited issue
MUST produce one bounded work record whose source version is the BLAKE3 of the
exact event bytes and whose sequence is the parsed issue update timestamp. A
closed issue MUST produce an explicit closed observation, never an implementation
completion. Unsupported actions and unrelated events MUST be ignored without
creating work.

#### Scenario: Authenticated issue becomes one work record

r[kiln.implementation.issue_intake.scenario.admit]
- GIVEN a valid signature, enrolled repository and enrolled actor
- WHEN an opened issue delivery is admitted
- THEN exactly one bounded work record MUST be produced
- AND its actor, repository, source alias and sequence MUST match the delivery

#### Scenario: Forged or unenrolled delivery is refused

r[kiln.implementation.issue_intake.scenario.refused]
- GIVEN an invalid signature, a mismatched repository, an unenrolled actor, or a malformed payload
- WHEN admission runs
- THEN it MUST fail before producing work

#### Scenario: Edits version and closing never completes

r[kiln.implementation.issue_intake.scenario.lifecycle]
- GIVEN an edited or closed issue delivery
- WHEN admission runs
- THEN the edit MUST produce a versioned work observation
- AND closing MUST produce an explicit closed observation, not completion
