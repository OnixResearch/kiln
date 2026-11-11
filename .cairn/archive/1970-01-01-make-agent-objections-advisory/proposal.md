## Why

Agent approvals are advisory by default, but agent `Veto`, `NeedsRevision`, and
blocking threads still block submission. There is no measured evidence that
adversarial agent objections improve changes, while the current behavior gives
them the same gate authority as human objections. Issue #35 asks for a
non-binding default.

## What Changes

- Under the default `agent_votes = advisory` policy, ignore agent vetoes and
  agent-opened blocking threads for the `vetoes` and `threads` checklist items.
- Retain those objections in a separate `Checklist.advisory` list with no
  authority; never count them in `ready`.
- Show the advisory list separately in the code-review submit panel.
- Keep explicit `agent_votes = binding` behavior unchanged.
- Do not change persisted review journals or policy bytes.

## Impact

- **Files**: `crates/kiln-core/src/change_review.rs`,
  `crates/kiln-web-service/src/code-review.js`.
- **Testing**: default-advisory and binding-mode checklist cases; agent vote
  and blocking-thread cases; full core, web-service and code-review tests.
- **Authority**: advisory objections cannot block or satisfy submission.
  Typed check evidence and agent comment outcome measurement from #35 remain
  separate follow-ups.
