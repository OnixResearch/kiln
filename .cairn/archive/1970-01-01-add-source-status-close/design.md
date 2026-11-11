## Decisions

The source plan binds an admitted GitHub issue alias, not free-form issue text.
Admission recomputes the integration identity and requires the publication record
to be integrated. The client reads first, closes only an open issue, and re-reads
before returning a receipt. Already closed issues are idempotent.

## Non-claims

Issue closure does not prove release eligibility, Cairn lifecycle completion or
source-history truth. No comment, reopen, label, pull-request or other issue
mutation is performed.
