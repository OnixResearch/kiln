## Why

Agent vote policy now distinguishes declared agent principals, but a patch-set
submission still records only self-declared authorship. Review needs to know
which exact authenticated principal submitted a revision when that principal is
declared as an agent, without inferring authorship from content.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Record `submitted_by` on a revision when the submitting principal is in the
  trusted review-profile agent list.
- Reject an agent principal that declares `human` authorship before mutation.
- Leave human and unattested submissions byte-compatible: the field is absent
  when the submitter is not an agent.
- Display the attested agent principal separately from self-declared authorship
  in the review identity view.
- Add positive and negative core tests and update the UI source.

This records the exact principal identity only. Agent harness/model metadata,
access-profile derivation and signed per-request agent credentials remain
future work.
