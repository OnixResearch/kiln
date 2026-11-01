## Context

`AuditRecord` is persisted with each admitted or denied boundary decision.
`Principal` now carries the optional access agent declaration.

## Decisions

- Store the same bounded declaration on the audit record and skip serialization
  when absent, preserving existing journals.
- Populate it only from the authenticated principal; anonymous or non-agent
  principals stay unchanged.
- Audit attribution has no effect on permissions, review votes, execution or
  merge authority.
- No separate agent-to-principal join is required for the activity view.

## Risks

- The harness/model are declared configuration, not verified execution.
- Historical audit records are not backfilled.
