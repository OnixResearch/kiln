## Context

The review state already records declared agent principals in `Facts.agents`.
Votes carry an actor and threads carry comments with authors. The checklist
currently filters votes only by reviewer eligibility and threads only by
`blocking`/`resolved`, so agent objections gate by default.

## Decisions

### Advisory default

The existing `agent_votes` policy is reused. In advisory mode, an agent vote
does not enter the `vetoes` item, and a blocking thread whose first comment was
written by a declared agent does not enter the `threads` item. Binding mode
keeps the existing filters.

### Separate display list

`Checklist` gains an optional `advisory` list containing one entry per ignored
agent veto and agent-opened blocking thread. It is serialized only when
non-empty and is never included in `items` or `ready`. The code-review page
renders it as a labelled non-binding subsection.

### No state or policy migration

No persisted structure changes. Existing votes, threads, journals, and policy
bytes continue to deserialize. Agent identity remains trusted shell input from
`Facts.agents`.

## Risks

- An agent thread opened by a human is treated as human and still blocks. This
  preserves explicit human escalation; it does not infer agent authorship from
  later comments.
- Typed check evidence and agent comment outcome rates are not implemented by
  this change; #35 remains partially open.
