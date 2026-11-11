## Why

Agent vote policy currently depends on a separately maintained review-profile
agent list, even though access already has the exact viewer principal. Operators
must duplicate identities and can drift.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add optional `agent = { harness, model? }` to viewer bindings.
- Validate bounded, trimmed, non-control harness/model text.
- Derive exact `viewer:<id>` principals from access bindings and merge them into
  the review profile's agent list when code review is configured.
- Keep existing review-profile agent entries for non-viewer or explicitly
  configured agents.
- Document that the declaration is configuration, not verified execution
  metadata, and that the review write-principal intersection is enforced at
  startup.

This derives agent identity from access. Recording harness/model on each
attested revision and signed per-request agent credentials remain future work.
