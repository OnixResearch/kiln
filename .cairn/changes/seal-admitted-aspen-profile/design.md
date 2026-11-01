## Context

`AdmittedAspenRuntimeProfile` carries the reviewed runtime profile and its
content identity. External consumers read the fields directly. The type has no
serializer; only struct construction and field mutation are open.

## Decisions

### Decision: Crate-private fields with accessors

**Choice:** Make `profile` and `profile_identity` `pub(crate)` and expose
`profile()` and `profile_identity()` returning references.

**Rationale:** External consumers retain read access without being able to
fabricate admission. No external consumer mutates the fields.

## Risks / Trade-offs

- This is a source-breaking API change for direct field access; workspace
  consumers and tests are updated.
- Accessors expose borrowed values; callers needing ownership clone them.
