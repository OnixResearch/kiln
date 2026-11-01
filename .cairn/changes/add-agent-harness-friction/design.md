## Context

`ReviewChangeFact.agent_submitted` marks any attested revision. Revision
`submitted_by.agent` carries a declared harness and optional model. Review
windows are bounded and already aggregate self-declared authorship classes.

## Decisions

- Add a bounded set of declared harnesses to each change fact; it is omitted
  when empty.
- Friction aggregation counts each change once per harness, not each revision.
- Agents without declared harness metadata are not assigned a synthetic harness.
- Encode the top-level map only when non-empty, preserving earlier zero-map
  identities.
- Render plain/HTML lists; the data remains observational and non-authoritative.

## Risks

- A change spanning multiple revisions/harnesses counts once per harness.
- Historical windows and revisions are not backfilled.
- Declared harness metadata is not verified execution.
