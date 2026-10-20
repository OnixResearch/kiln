# Authorization Reference: Resource Scope Semantics

This reference records how the verified capability helpers authorize a
resource. It documents the measured byte-prefix containment rule and the
delimited resource-construction guidance. It does not redefine UCAN, token,
DID, caveat, proof, or policy semantics.

## Byte-prefix containment

Resource containment in this crate is byte-prefix matching. A capability
scoped to one resource authorizes every resource that begins with those
exact bytes.

The verified helper `capability_set_authorizes` accepts a request when any
supplied capability has a resource byte-prefix that contains the requested
resource and an ability that exactly equals the requested ability. The
helper does not perform wildcard ability subsumption.

A scope covers every string that starts with its bytes. This rule is
intended UCAN semantics. One consequence is that a scope can authorize more
than the exact string that the issuer wrote.

### Nested resource example

A capability scoped to `did:key:z6Mkresource` authorizes
`did:key:z6Mkresource/photos`. The nested string begins with the scope
bytes.

### Shared-prefix overmatch example

A capability scoped to `did:key:z6Mkresource` also authorizes
`did:key:z6MkresourceX`. The string `did:key:z6MkresourceX` is a sibling
value that shares the scope bytes without a delimiter. This behavior is
the shared-prefix overmatch.

A capability scoped to `did:key:z6Mkresource` does not authorize
`did:key:z6Mkother`. That string does not begin with the scope bytes.

## Resource-construction guidance

Construct resources with explicit segment delimiters. The `/` separator
marks each namespace segment. Use one delimiter scheme in the whole
resource.

Example delimited form:

```text
did:key:z6Mkresource/photos
```

The issuer names the principal, then a `/`, then the path segments. Each
segment is distinct.

Do not concatenate parts without a delimiter. Two values that merge into
one prefix create an accidental sibling. The merged resource begins with
the scope bytes, so a narrower capability can authorize it.

Example of delimiter-free concatenation to avoid:

```text
"did:key:z6Mkresource" + "photos"   # produces did:key:z6Mkresourcephotos
```

The value `did:key:z6Mkresourcephotos` begins with the scope bytes of
`did:key:z6Mkresource`. A capability scoped to `did:key:z6Mkresource`
covers it. With the `/` separator, `did:key:z6Mkresource/photos` is the
intended nested resource and no accidental sibling exists.

## Pinned fixtures

The `verified` test module records these results as pins. Each pin names
the semantics it locks in place.

| Capability scope | Requested resource | Result |
|---|---|---|
| `did:key:z6Mkresource` | `did:key:z6Mkresource/photos` | Authorized |
| `did:key:z6Mkresource` | `did:key:z6MkresourceX` | Authorized |
| `did:key:z6Mkresource` | `did:key:z6Mkother` | Not authorized |

A change that alters the byte-prefix containment rule fails these pins.

## Boundary

This reference records measured helper behavior only. It does not claim
evaluator equivalence, cryptographic correctness, policy correctness,
wildcard containment replacement, or release readiness.
