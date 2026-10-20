## Context

`VerifiedCapability::contains` uses verified scope and UCAN ability subsumption for pairwise containment. Separately, the vendored `ucan_capability_merge` family proves finite capability-set authorization with resource byte-prefix containment and exact ability equality.

## Decisions

### Data-only helper

Expose `capability_set_authorizes(capabilities, resource, ability) -> VerifiedResult<bool>` as an advanced helper over validated `CapabilityDocument` values plus a concrete requested resource/ability pair.

### Exact semantics

The helper intentionally mirrors `ucan_capability_merge::check_authorized_by`: finite existential authorization, resource prefix refinement, and exact ability equality. It does not use wildcard ability subsumption. Callers that need full UCAN token/effective-delegation semantics must keep using shell authorization APIs.

### Boundary

The helper performs no compact-token parsing, proof resolution, cryptographic signature verification, caveat policy evaluation, revocation backend checks, networking, persistence, replay protection, or ambient clock reads.

## Risks

- **Semantic confusion:** exact ability equality is narrower than `VerifiedCapability::contains` wildcard subsumption. Mitigate through API docs, tests, and OpenSpec boundary language.
