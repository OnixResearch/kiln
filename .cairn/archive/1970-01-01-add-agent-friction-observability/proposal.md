## Why

Agent attestation now exists on revisions and submission receipts, but the
review-friction observation only reports self-declared authorship classes. An
operator cannot measure how much reviewed work carries attested agent identity
without inspecting each change.

Issue: [#24](https://github.com/OnixResearch/kiln/issues/24) and
[#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Mark a review window change when any admitted revision carries an attested
  agent submission.
- Count those changes per self-declared authorship class as
  `agent_submitted_changes`.
- Include the count in observation identity only when non-zero, extending the
  trailing tag encoding without changing earlier zero-count identities.
- Display the count in the dashboard review-friction table.
- Add focused observation and dashboard tests.

Attestation remains declared principal identity, not verified execution, and the
new count carries no release or review authority.
