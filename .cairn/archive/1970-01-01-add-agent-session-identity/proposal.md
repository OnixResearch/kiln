## Why

Agent declarations now drive review policy and attestation, but the admitted
session principal does not expose whether it is an agent. Operators and the
workspace cannot identify an agent identity outside a specific revision or vote.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Carry the optional access agent declaration on the authenticated principal.
- Include the declaration in `/api/session` when present and omit it otherwise.
- Show the declared harness/model in workspace principal facts as configuration,
  not verified execution or authority.
- Add a session regression test and document the surface.

No access, review, execution or merge authority changes. Tailnet and viewer
principals use the declarations added by access agent binding support.
