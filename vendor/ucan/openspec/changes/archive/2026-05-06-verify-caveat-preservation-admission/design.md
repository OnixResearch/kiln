## Context

`capability_is_delegated` currently combines `VerifiedCapability::contains` with `parent_caveats_are_preserved`. The first half is already verified, while the second half is ordinary Rust that scans parent and child `CaveatDocument` values for exact equality over domain, type, key, and payload. The shell also has a stable length-prefixed caveat identifier encoder used by proof traversal transcripts.

## Goals

- Verify the finite preservation predicate: every parent caveat identifier appears byte-for-byte in the child identifier list.
- Fail closed during attenuation when a child drops or changes an opaque parent caveat.
- Preserve existing support for child-added caveats.
- Keep caveat-document validation, length-prefixed identifier construction, payload parsing, and policy semantics outside the verified claim.

## Non-Goals

- Do not verify arbitrary caveat policy semantics, typed caveat parsing, length-prefix encoder injectivity, serde/provenance, clocks, counters, revocation, proof loading, or cryptographic operations.
- Do not change the existing caveat identity shape or require ordered child caveats.

## Decisions

### Identifier bytes as the bridge boundary

The shell converts parent and child caveats to the existing length-prefixed identifier bytes, then passes `Vec<Vec<u8>>` lists to the bridge. The verified predicate checks unordered finite preservation over those bytes. This matches the existing semantic branch while avoiding claims about how identifiers were constructed.

### Preservation remains paired with capability containment

The bridge is called only after `VerifiedCapability::contains` succeeds. The result is still a conjunct in delegation acceptance: a child capability must be resource/ability-contained and must preserve every opaque parent caveat identifier.
