## Why

Agent identity is now authenticated, shown in review, and attached to
submissions, but attributable audit records still show only principal id, label
and role. Operators cannot tell whether a denied or admitted request came from a
declared agent without joining other state.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add the optional declared agent identity to audit records.
- Populate it from the authenticated principal, omitting it for non-agents.
- Extend the denied-viewer test to assert the audit attribution.
- Document the audit field.

This is attribution, not authority or execution proof. Existing audit records
without the field remain valid and byte-compatible.
