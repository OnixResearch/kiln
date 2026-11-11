## Context

`DefeloPatch` currently admits only `commits`. `map_defelo_request` resolves the
first commit for both push and patch events but sets `patch: None`. The older
line-JSON `BrokerTrigger` already maps a patch ID.

## Decisions

### Optional request field, required patch fact

`DefeloPatch.id` is optional so existing push fixtures and patch envelopes
without identity remain deserializable. A patch event without an exact patch ID
fails mapping with `radicle_patch_identity`; a push event ignores the field.
The patch ID is bounded by the existing `ExternalRef` admission rules.

### No trust inference

A preserved patch ID is producer-owned identity. It does not establish author
signature, patch approval, or source availability. Those remain later #18
slices.

## Risks

- The deployed broker version may omit patch IDs. Those patch events now fail
  closed instead of producing an unbound run.
- This change does not implement patch source acquisition or approval gating.
