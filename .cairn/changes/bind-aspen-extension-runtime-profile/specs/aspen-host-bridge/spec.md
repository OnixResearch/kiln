# Aspen Host Bridge Specification Delta

## Added Requirements

### Requirement: Exact extension runtime profile binding

r[kiln.aspen_host.extension_profile_binding] The production Aspen host MUST bind every extension callback execution to the same exact admitted runtime profile used for host ingress.

#### Scenario: Matching non-default profile is accepted

r[kiln.aspen_host.extension_profile_binding.accepted]
- GIVEN the host admits an explicit runtime profile with a non-default positive bound
- AND the host supplies its canonical path and exact BLAKE3 profile identity to the extension process
- WHEN the extension admits the same profile document
- THEN it MUST handle the callback with that admitted profile
- AND lifecycle, trigger, and recovery meaning MUST use one profile identity

#### Scenario: Profile input is absent or differs

r[kiln.aspen_host.extension_profile_binding.rejected]
- GIVEN the profile arguments or document are missing, duplicated, malformed, unreadable, oversized, invalid, changed, or identity-mismatched
- WHEN extension shell admission runs
- THEN execution MUST fail before callback handling
- AND it MUST NOT select the compiled fixture, another profile, or another runtime
- AND an already reserved operation MUST remain `Unknown`
- AND reconciliation MUST NOT redispatch that operation
