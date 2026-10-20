## ADDED Requirements

### Requirement: Verified typed caveat policy bridge
The library SHALL provide a first-party caveat policy set for the exact finite typed caveat subset backed by vendored verified logic, covering time-bound and rate-limit caveats over caller-supplied runtime facts.

#### Scenario: Satisfied typed caveats admit authorization
- **WHEN** a matching capability carries recognized typed caveats whose parsed facts are satisfied by the caller-supplied caveat context
- **THEN** authorization through the typed policy set SHALL allow that capability when resource and ability attenuation also match

#### Scenario: Unsatisfied or malformed typed caveats deny authorization
- **WHEN** a matching capability carries a recognized typed caveat whose parsed facts are unsatisfied or malformed
- **THEN** authorization through the typed policy set SHALL fail closed with a caveat rejection

#### Scenario: Unknown caveats remain caller controlled
- **WHEN** a matching capability carries a caveat outside the typed policy identities
- **THEN** the typed policy set SHALL NOT silently satisfy it and SHALL preserve the existing missing-policy/deny-by-default behavior unless another caller-supplied policy handles it

#### Scenario: Runtime facts remain trusted shell inputs
- **WHEN** the typed policy set evaluates time or rate-limit caveats
- **THEN** it SHALL document that current time, use counts, and external quota/freshness/provenance are caller-supplied runtime facts outside the verified primitive
