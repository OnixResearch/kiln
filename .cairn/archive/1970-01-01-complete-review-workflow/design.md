## Context

The existing provider-neutral review core and exact-target local Git adapter already retain private drafts, immutable revisions, approvals, operation journals and canonical CI facts. Extend those conventions rather than creating another review store or CI verdict channel.

## Goals and Success Contract

Implement ownership/protection, contribution intake, review ergonomics, mentions/subscriptions/external delivery and a CI-bound merge queue end to end. A successful required-CI review must reach an independently observed disposable target; stale revisions, moved targets, unapproved owners, malformed intake, ambiguous effects and unsafe delivery must fail closed. Desktop/mobile users must access real results.

## Decisions

### Decision: Explicit review policy and target authority

**Choice:** Extend pure review policy with path/team owners and required approvals; automatically request eligible owners. Make the configured target writer boundary visible and enforce it for every service mutation. Document that unmanaged OS/repository writers require external access control rather than claiming the service controls them.

**Rationale:** Service checks cannot authorize or prevent unrelated writers; adapter capabilities and exact CAS remain mandatory.

### Decision: Reuse journals and immutable identities

**Choice:** Source intake resolves admitted exact source identities and routes stable branch mappings through existing review registration. Suggestions and queue candidate creation use versioned provider effects, durable reservation and exact reconciliation. Queue submissions retain enqueue authority and recheck policy, revision, target and canonical candidate CI before effects.

**Rationale:** A new commit needs its own CI evidence and cannot inherit an old source verdict. Uncertain effects cannot be blindly retried.

### Decision: Isolated modules and thin integration

**Choice:** Keep ownership semantics in the existing pure core. Put intake, notification delivery and queued workflow in focused shell modules backed by existing SQLite state. Provider adapter owns commit construction; browser owns safe rendering and controls. The integration owner alone changes shared command routing and server lifecycle.

**Rationale:** Distinct edit ownership prevents conflicting parallel changes while maintaining one authoritative state and port vocabulary.

### Decision: Safe review rendering and delivery

**Choice:** Render Markdown and highlighting without unsanitized HTML. Whitespace views retain exact anchor coordinates. Suggestions bind exact revision/path/range and produce new revisions with invalidated approvals. Only published comments generate mentions; subscription preferences are principal-owned. Delivery uses explicitly admitted destinations, bounded execution and durable identities, never private drafts.

**Rationale:** Presentation cannot alter authority, expose unpublished data or turn review text into code execution or arbitrary outbound requests.

## Risks / Trade-offs

- Queue candidate reconstruction on target movement invalidates CI and must stop/rebuild rather than reuse success.
- Branch identity and author attribution must not come from untrusted browser assertions.
- Notification transport can be ambiguous; record delivery identity and outcome without asserting exactly-once recipient effects.
- External branch protection depends on deployment access control and cannot be manufactured by a UI switch.

## Verification Strategy

After concurrent edits finish, run focused pure and adapter tests plus real disposable HTTPS/browser workflows, actual CI candidate execution, notification receiver readback and crash/restart recovery. Then run formatting, workspace tests, Clippy, Nickel, lifecycle and host Nix gates once. Retain exact results and non-claims; do not publish or archive.
