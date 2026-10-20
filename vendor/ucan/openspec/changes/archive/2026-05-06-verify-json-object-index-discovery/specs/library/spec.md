## MODIFIED Requirements

### Requirement: Verified compact-token codec boundary [r[library.verified-compact-token-codec-boundary]]

The library MUST route compact-token split, framing, proof-reference digest, base64url segment, decoded JSON object structural admission, decoded JSON object significant-index discovery, and parsed header field admission through vendored verified-logic predicates before continuing into trusted runtime codec, JSON parser, structural-validation, or cryptographic boundaries.

#### Scenario: Decoded JSON object admission discovers significant indexes with verified logic [r[library.verified-compact-token-codec-boundary.scenario.json-significant-index-discovery]]

- GIVEN decoded compact-token header or payload bytes with leading and trailing RFC-8259 JSON whitespace around a top-level object envelope
- WHEN the library checks decoded JSON object structural admission before serde parsing
- THEN the library MUST route first/last significant-byte discovery and object-envelope admission through vendored verified logic
- AND the library MUST NOT rely on UCAN-local unverified significant-index discovery as the admitted fact
- AND the library MUST continue to reject empty, whitespace-only, array, scalar, or non-object-envelope decoded bytes before serde parsing

#### Scenario: JSON index discovery keeps parser and codec non-claims explicit [r[library.verified-compact-token-codec-boundary.scenario.json-index-discovery-non-claims]]

- GIVEN decoded JSON object bytes that pass the verified significant-index and envelope predicate
- WHEN the library subsequently parses JSON fields, decodes base64url segments, validates signatures, derives proof references, resolves proofs, evaluates caveats, checks revocations, or reads evaluation time
- THEN the library MUST continue to document JSON parser correctness, UTF-8/string extraction, duplicate-key behavior, canonicalization, base64url decoder correctness, cryptographic correctness, backend truth/freshness, caveat policy truth, clock provenance, and token provenance as outside the verified JSON index-discovery claim

### Requirement: Verified bridge coverage sweep [r[library.verified-bridge-coverage-sweep]]

The repository MUST keep a guard-readable inventory of public verified exports, live verifier bridge calls, and vendored UCAN/WOT verified-logic modules, and MUST classify each item as `live-path`, `public-helper`, `candidate`, or `documented-boundary`. Exact executable deterministic/data-only candidates MUST either be routed into the appropriate live path or intentionally documented as boundaries before the sweep is complete, and the repository MUST NOT preserve vendored source repository metadata, credentials, compact token values, private keys, bearer tokens, or other secret material in the inventory.

#### Scenario: JSON index-discovery bridge is tracked as live path [r[library.verified-bridge-coverage-sweep.scenario.json-index-discovery-live-path]]

- GIVEN the vendored verified-logic snapshot includes an executable decoded JSON significant-index discovery or direct object-envelope admission helper
- WHEN UCAN routes decoded header/payload JSON admission through that helper before serde parsing
- THEN the bridge coverage inventory MUST classify the public UCAN bridge and vendored module coverage as `live-path`
- AND the bridge guard MUST fail if the new public bridge or vendored UCAN JSON module coverage disappears from the inventory
