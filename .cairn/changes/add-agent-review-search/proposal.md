## Why

Agent attestation is visible on revisions and receipts, but reviewers cannot
filter the change list for work submitted by attested agent principals.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add optional `agent` to the review search query.
- Filter on whether the current revision carries an attested agent submission.
- Accept `agent=with|without` in the review query and expose the same filter in
  the console search form and saved views.
- Add core search and web query tests.

Filtering is read-only and confers no review, execution or merge authority.
