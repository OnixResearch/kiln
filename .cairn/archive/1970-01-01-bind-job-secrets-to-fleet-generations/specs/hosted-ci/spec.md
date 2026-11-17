# Hosted CI Delta

## ADDED Requirements

### Requirement: Fleet-bound job-secret generation

r[kiln.hosted.fleet.secret_generation] A hosted credential-bearing job MUST bind its existing secret capability and workload token to the operator-selected active fleet instance, revision and original provisioning request identity. Secret admission, delivery, token exchange and resource-server validation MUST refuse an unknown, non-active, retired, replaced or mismatched generation. An observed retirement MUST revoke matching job authority atomically with the fleet state transition. An uncertain teardown MUST leave the credential unusable without falsely recording completed revocation. Expiry and the binding MUST survive reopening the retained database. The fake provider qualification MUST inspect those boundaries per worker generation without granting physical allocation authority to the pure CI core or claiming production-provider qualification.

Typed Nickel execution profiles MUST require a bounded positive fleet generation selector for every workload granting an audience or secret, and MUST reject missing or malformed selectors. The selector MUST equal the currently active durable row; capability and JWT binding values MUST come from the observed SQLite row rather than the operator-authored selector alone.

#### Scenario: Typed operator selection is insufficient without an active row
r[kiln.hosted.fleet.secret_generation.scenario.selection]

- GIVEN a credential-bearing workload with a typed fleet selector, or an absent/invalid selector
- WHEN the operator profile is admitted and the executor reads the durable fleet row
- THEN malformed/missing selectors are refused and a well-formed selector grants nothing unless it matches an active observed generation.

#### Scenario: Current generation delivers and expires
r[kiln.hosted.fleet.secret_generation.scenario.current]

- GIVEN an operator-selected active worker generation and an admitted credential-bearing job
- WHEN the executor delivers its secret and exchanges and validates its token
- THEN the capability and claims name that exact generation, and the delivery and token expire at their bounded deadlines.

#### Scenario: Replaced and unknown generations fail closed
r[kiln.hosted.fleet.secret_generation.scenario.stale]

- GIVEN a missing, stale or replaced worker generation or a stale provider receipt
- WHEN secret admission, delivery, exchange or resource-server validation is attempted
- THEN credentials for the old or unknown generation cannot be issued or accepted.

#### Scenario: Observed retirement revokes while uncertainty does not complete revocation
r[kiln.hosted.fleet.secret_generation.scenario.retirement]

- GIVEN an active worker with issued credentials
- WHEN deprovisioning becomes uncertain or an absent provider receipt observes retirement
- THEN uncertain credentials are unusable but not falsely marked revoked-complete, and observed retirement atomically revokes old job authority.

#### Scenario: Restart preserves binding
r[kiln.hosted.fleet.secret_generation.scenario.restart]

- GIVEN a bound job and token retained in SQLite
- WHEN the coordinator reopens its database and the worker generation changes or expires
- THEN validation uses the retained binding and current fleet row, never treating old credentials as current.
