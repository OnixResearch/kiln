## Context

`review_window` already folds revisions into `ReviewChangeFact` and explicitly
avoids inferring authorship. Revision `submitted_by` is the trusted attested
principal from the review agent set. Friction views aggregate per declared
authorship class and encode a stable identity.

## Decisions

- Add an optional `agent_submitted` change fact, defaulting false and omitted
  from serialization when false.
- Count changes with any attested revision, independent of declared authorship;
  the class dimension stays self-declared.
- Add `agent_submitted_changes` with the existing zero-omitted shape.
- Extend the friction identity trailing option with a separate agent tag bit and
  value only when non-zero, preserving prior identities for zero counts.
- Render one dashboard column; no control, authority or merge effect.
- Keep the observation basis labelled self-declared/unverified in the existing
  view metadata; agent attestation is a separate principal fact.

## Risks

- A change with multiple revisions is counted once.
- Historical windows are not backfilled.
- Attested identity does not prove which harness/model executed.
