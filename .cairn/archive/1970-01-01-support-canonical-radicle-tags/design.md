## Context

A source request names one signed reference and optionally the same canonical
reference. Branch targets are commits. An annotated tag target is a Git tag
object whose `peel_to_commit` is the buildable revision. The contract currently
requires only `refs/heads/`, and both signed-ref and canonical verification
compare the raw object identity to `expected_revision`.

## Decisions

### Decision: `expected_revision` is the buildable commit

**Choice:** Keep `expected_revision` as the commit. For a tag reference, peel
the target object to a commit before comparing it to `expected_revision`;
retain the original tag object and `object_kind = "tag"` in canonical evidence.

**Rationale:** Snapshots, receipts and source resolutions remain commit-bound,
while release evidence still records the exact canonical tag object. This avoids
changing the receipt's snapshot revision semantics.

### Decision: Accept only heads and tags

**Choice:** Admit `refs/heads/` and `refs/tags/` references, retaining the
existing `..` and backslash refusals and the requirement that signed and
canonical references match.

**Rationale:** Those are the reviewed release and branch surfaces; arbitrary
Radicle namespaces remain out of scope.

## Risks / Trade-offs

- Tag peeling must reject a tag object that does not peel to a commit.
- The alias for a tag strips `refs/tags/`, matching the existing branch alias.
- Delegates remain the canonical-rule authority; this change does not add a
  trust shortcut.
