# Aspen Runtime Backend Delta Specification

## ADDED Requirements

### Requirement: Host content publication

r[kiln.aspen_runtime.content_publication] The Aspen host SHOULD open a bounded
Molten content store beside its durable state root and, when configured, publish
each successful effect output as exact bytes under its BLAKE3 reference before
the effect returns. Publication failure MUST fail the effect rather than claim
success. Reads MUST verify exact identity and enforce their byte bound; the store
MUST round-trip across reopen and support live Iroh publication without granting
retention, confidentiality or independent-verification claims.

#### Scenario: Exact output is published

r[kiln.aspen_runtime.content_publication.scenario.publish]
- GIVEN a configured host content store
- WHEN an effect succeeds with exact output bytes
- THEN those bytes MUST be retained under their BLAKE3 reference before success

#### Scenario: Publication or read bound fails

r[kiln.aspen_runtime.content_publication.scenario.refused]
- GIVEN a content write failure or a read beyond its bound
- WHEN the effect or read runs
- THEN it MUST fail without returning unverified content

### Requirement: Bounded non-authoritative gossip hints

r[kiln.aspen_runtime.gossip] Kiln MAY exchange gossip hints that announce
content or state availability. Hints MUST be versioned, bounded in message size,
content size, lifetime, event count, recent-hint count and join time, and MUST
carry an exact schema, scope, sender, positive epoch, expiry and optional
BLAKE3 content length. Malformed, expired, oversized or duplicate hints MUST be
ignored or refused. Topic derivation from public scope text MUST NOT be claimed
as confidentiality. A hint MUST NOT grant authority, reserve work, prove
delivery or execute an effect; receivers MUST fetch and validate canonical state
independently.

#### Scenario: Bounded hint is delivered

r[kiln.aspen_runtime.gossip.scenario.live]
- GIVEN two loopback Iroh endpoints sharing a derived topic
- WHEN one sends a valid bounded hint
- THEN the listener MUST decode and deliver it exactly once
- AND the hint MUST remain non-authoritative

#### Scenario: Malformed or oversized hints are refused

r[kiln.aspen_runtime.gossip.scenario.bounds]
- GIVEN oversized messages/content, expired lifetime, invalid schema, zero epoch or duplicate hints
- WHEN decoding or listening
- THEN invalid hints MUST be refused or ignored under the profile bounds
