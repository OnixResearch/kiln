# Radicle Ci Compatibility Specification

## ADDED Requirements

### Requirement: Exact patch trigger identity

r[kiln.radicle.broker_031_wire.patch] The Defelo adapter MUST preserve an
admitted patch event's exact native patch identity in the typed trigger and
MUST reject a patch event without that identity. The first patch commit MUST
remain the exact Kiln revision. This mapping MUST NOT imply source trust,
author signature verification, approval, or patch execution authority.

#### Scenario: Patch envelope carries identity

- GIVEN a valid patch event with an exact patch ID and non-empty commit list
- WHEN Defelo mapping runs
- THEN the trigger MUST carry the patch ID and first commit without changing
  repository, branch, event, or actor projection.

#### Scenario: Patch identity is missing

- GIVEN a patch event without a patch ID
- WHEN Defelo mapping runs
- THEN mapping MUST fail before any provider or runtime effect.
