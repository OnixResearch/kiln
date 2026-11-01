## Context

`AdmittedJournal` carries a replayed state, canonical journal records, optional
schema and declared identity, and an optional review window. External tests and
the shell currently rely on public fields. The type has no serializer, so JSON
deserialization is already unavailable; only struct construction and field
mutation are open.

## Decisions

### Decision: Crate-private fields with read-only accessors

**Choice:** Change every field to `pub(crate)` and add accessors returning
borrowed state, records, schema, identity and review. Keep the type public so
consumers can use admitted journals without constructing them.

**Rationale:** External crates retain read access but can no longer fabricate an
admission result or mutate a reviewed journal. Read access remains sufficient for
reporting and rendering.

### Decision: Named review attachment

**Choice:** Replace direct `review` mutation with consuming `with_review`.

**Rationale:** The shell reads a review window after journal admission and must
attach it without exposing a writable field. The named operation keeps that one
transition explicit and testable.

## Risks / Trade-offs

- This is a source-breaking API change for any external field access; the
  dashboard tests are updated and no other repository consumer accesses the
  fields.
- `with_review` can replace an already attached window; it is an internal shell
  capability, not an admission bypass.
