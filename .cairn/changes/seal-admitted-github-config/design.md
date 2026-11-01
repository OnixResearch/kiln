## Context

`AdmittedGithubConfig` carries the reviewed service config, service and pipeline
identities, Lattice profile and transport profile. Hosted connections clone and
re-admit the config under a namespace. External consumers read fields directly.
The type has no serializer; only struct construction and field mutation are open.

## Decisions

### Decision: Crate-private fields with accessors

**Choice:** Make all five fields `pub(crate)` and expose `config`, `identity`,
`pipeline_identity`, `lattice` and `transport` accessors returning references.

**Rationale:** External consumers retain read access without fabricating
admission. Hosted re-admission clones the admitted config and calls
`admit_config` again, preserving validation.

## Risks / Trade-offs

- This is a source-breaking API change for direct field access; all workspace
  consumers are updated.
- Accessors expose borrowed values; callers needing ownership clone them.
