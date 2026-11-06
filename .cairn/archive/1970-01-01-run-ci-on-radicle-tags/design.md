## Context

`ProjectWorkerProfile` admits one configured source reference and turns a
broker source into a `TriggerRequest`. It currently requires `refs/heads/`,
strips the branch prefix and always emits `Push`. The source shell already
supports exact canonical tag acquisition.

## Decisions

### Decision: Add an explicit `Tag` trigger event

**Choice:** Add `TriggerEventKind::Tag`. Use it when the configured source
reference is under `refs/tags/`; keep `Push` for `refs/heads/`.

**Rationale:** A tag source is a distinct release event, not a branch push.
Downstream consumers can distinguish it without changing existing push events.

### Decision: Alias is the tag name

**Choice:** Strip `refs/tags/` for the trigger alias, matching branch alias
handling.

**Rationale:** The alias remains a short display/identity value; the exact
reference and revision remain in the source profile and trigger.

## Risks / Trade-offs

- Existing push triggers and serialized events are unchanged.
- The broker does not yet emit tag events; this profile is for exact local or
  future broker tag intake.
- Tag CI inherits the existing source and executor authority; no new approval
  semantics are added.
