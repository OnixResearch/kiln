# Hosted CI Delta

## ADDED Requirements

### Requirement: Fleet lifecycle qualification

r[kiln.hosted.fleet.lifecycle_qualification] Fleet provisioning and reaping MUST be qualifiable through the unchanged immutable-executable admission with a fake provider that keeps a durable inventory and operation log, real coordinator processes and the real SQLite state. Interrupting the coordinator before or after a provider effect, losing an acknowledgement, redelivering a request, returning a stale receipt or changing the admitted operator generation MUST reconcile by the original request identity before any repeated effect. Unknown and present instances MUST keep their capacity until reconciliation observes an allowed terminal state. A retired instance MUST NOT become active again from a reappearing provider resource or a stale receipt. An inventory oracle MUST detect duplicate creation and premature slot release. Receipts MUST keep `production_provider_qualified=false`.

#### Scenario: Interrupted effects reconcile without repetition

- GIVEN a coordinator killed while the driver waits before or after its effect, or a driver that loses its acknowledgement
- WHEN the same request is delivered again and the instance is reconciled
- THEN no second provider effect occurs, the uncertain instance keeps its capacity, and reconciliation records the provider's actual presence or absence.

#### Scenario: Stale and retired state cannot resurrect

- GIVEN a stale receipt, a changed operator driver generation or a retired instance whose provider resource reappears
- WHEN new effects or reconciliation are requested
- THEN the stale observation is not accepted, new effects require reconfiguration, and the retired instance stays retired.

#### Scenario: Conservation controls are detected

- GIVEN a deliberate duplicate creation or a premature retirement while the provider still holds the resource
- WHEN the inventory oracle compares fleet rows with provider records
- THEN the violation is reported, while ordinary scenarios report none.
