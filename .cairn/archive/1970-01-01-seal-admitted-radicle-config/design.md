## Context

`AdmittedRadicleConfig` carries the reviewed shell config, Lattice profile and
transport profile. External consumers move or borrow these fields directly.
The type has no serializer; only struct construction and field mutation are
open.

## Decisions

### Decision: Crate-private fields with accessors

**Choice:** Make the three fields `pub(crate)` and expose `shell`, `lattice` and
`transport` accessors returning references. Consumers clone owned values where
they previously moved a field.

**Rationale:** External code keeps read access without being able to fabricate
admission. Cloning preserves the existing ownership flow without exposing a
writable field.

## Risks / Trade-offs

- This is a source-breaking API change for direct field access; all consumers
  in this workspace are updated.
- Accessors expose borrowed references; callers must clone when they need owned
  values.
