# Prevent reclaim of verified work versions

## Why

Verification clears the active attempt claim but leaves work ranked and budget
remaining. A scheduler could then create another attempt for already verified
work.

## What Changes

- Refuse a claim when the exact work identity/revision already has a verified
  attempt.
- Keep a different source version separately claimable.
- Add a regression and document the terminal semantics.

## Impact

- **Files**: attempt ledger, implementer claim path, docs and specification.
- **Testing**: same-version reclaim refused after verification.
- **Authority**: claim admission only; no dispatch, publication or completion.
