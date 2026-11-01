## Context

`Principal` is the authenticated access identity serialized by `/api/session`.
Viewer and tailnet bindings may declare an agent; review profiles derive their
agent set from those declarations.

## Decisions

- Add the optional declaration to `Principal` and skip serialization when absent,
  so human/operator/worker sessions keep their existing shape.
- Viewer and tailnet authentication copy the exact binding declaration; synthetic
  automation/system principals remain non-agent.
- The session exposes declared identity only. Audit authority, permissions and
  review votes remain unchanged.
- The web workspace renders the declared harness/model in principal facts with a
  declared label.

## Risks

- This is identity configuration, not proof of the executing model.
- Adding a field to the session JSON is additive; consumers that ignore it keep
  working.
