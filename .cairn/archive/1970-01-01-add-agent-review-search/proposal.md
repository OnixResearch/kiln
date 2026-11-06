## Why

Agent attestation is visible on revisions and receipts, but reviewers cannot
filter the change list for work submitted by attested agent principals.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add optional `agent` to the review search query.
- Filter on whether the current revision carries an attested agent submission.
- Add optional exact `agent_harness` and `agent_model` filters for the declared
  identity on the current attested revision.
- Accept `agent=with|without` and bounded harness/model values in the review
  query and expose the same filters in the console search form.
- Preserve every agent filter when saving and applying personal views, and clear
  them together.
- Add core search, saved-query and web query tests.

Filtering is read-only and confers no review, execution or merge authority.
