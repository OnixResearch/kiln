## ADDED Requirements

### Requirement: Canonical delegate source mode

r[kiln.radicle.canonical_source]
A source request MAY carry `canonical = { identity_commit, reference }` whose
reference equals the requested signed branch. In canonical mode the shell MUST
verify every delegate of the pinned identity document with the existing
signed-refs checks, require each observing delegate's signed refs to embed
identical identity-document bytes, compute the canonical head for the reference
with the vendored heartwood implementation using the document's
`xyz.radicle.crefs` rules (or its delegate threshold when no rule matches), and
accept the requested revision only when it equals that head. Insufficient votes
and diverging tips MUST be refused. Requests and receipts without `canonical`
MUST keep their previous bytes.

#### Scenario: Two of three delegates agree

r[kiln.radicle.canonical_source.scenario.quorum]
- GIVEN a pinned identity document with three delegates and threshold two, where
  two delegates sign the same tip for the configured reference
- WHEN the canonical request names that tip
- THEN the source MUST resolve and acquire it, AND the receipt MUST record the
  identity commit and root, threshold two, three delegates and two observations

#### Scenario: A single vote is insufficient

r[kiln.radicle.canonical_source.scenario.insufficient]
- GIVEN the same document with threshold two and one delegate signing the tip
- WHEN the canonical request names that tip
- THEN the source MUST be refused with `radicle_source_canonical_quorum`, AND no
  receipt or run MUST be produced

#### Scenario: Diverging tips have no canonical head

r[kiln.radicle.canonical_source.scenario.diverging]
- GIVEN four delegates with threshold two, where two sign one tip and two sign
  another
- WHEN the canonical request names either tip
- THEN the source MUST be refused with a canonical quorum or object failure

### Requirement: Canonical receipt evidence

r[kiln.radicle.canonical_source.evidence]
A canonical receipt MUST list the identity commit, identity root, threshold,
delegate DIDs, reference, canonical object and object kind, and each observing
delegate's verified object. Admission MUST reject evidence without a canonical
request, a canonical request without evidence, an object other than the requested
revision, a reference other than the signed branch, duplicate delegates, a
threshold of zero or above the delegate count, and observations by non-delegates.

#### Scenario: Contract rejects unlinked evidence

r[kiln.radicle.canonical_source.evidence.scenario.links]
- GIVEN the canonical receipt fixture
- WHEN evidence is present without the request's canonical inputs, or a
  threshold, reference or observing delegate is changed
- THEN Nickel and Rust admission MUST reject the receipt

### Requirement: Pinned identity revision non-claim

r[kiln.radicle.canonical_source.identity]
The identity document revision MUST be an operator-pinned input. The shell MUST
read the document at that commit and require delegates to embed identical bytes,
but MUST NOT claim to verify the document's own revision history or threshold
acceptance. The documentation and non-claims name this boundary.

#### Scenario: A delegate with a different document is refused

r[kiln.radicle.canonical_source.identity.scenario.revision]
- GIVEN a delegate whose signed refs embed a different identity document revision
- WHEN canonical verification runs
- THEN it MUST fail with `radicle_source_canonical_identity_revision`
