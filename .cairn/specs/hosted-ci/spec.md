# Hosted Ci Specification

## Purpose

Defines the `hosted-ci` capability.

## Requirements

### Requirement: Iroh customer connection
r[kiln.hosted.iroh] Customer CLI, agent and browser-bridge access MUST use authenticated Iroh with bounded versioned framing, explicitly selected relays and server-side current peer authorization. Caller-supplied organization/principal fields MUST NOT establish authority. Browser bridges MUST bind only loopback, enforce origin/host protections and keep private keys local.

#### Scenario: Authorized and unauthorized peers
r[kiln.hosted.iroh.scenario.peers] An enrolled peer can exercise only its current organization/project scopes; an unknown or revoked peer and malformed/oversized requests are rejected before application effects.
- GIVEN enrolled and untrusted peers
- WHEN each sends a bounded request
- THEN only current authorized scopes may reach application effects.

### Requirement: External source connections
r[kiln.hosted.connections] An authorized customer MUST connect an existing externally hosted repository with stable native identity, scoped credential references, generation and reviewed exact workflow binding. Native adapter replay, exact-source acquisition and upstream status semantics MUST be reused. Disconnect MUST deny new admissions without deleting upstream source or erasing replay records.

#### Scenario: Replay and changed authority
r[kiln.hosted.connections.scenario.revoke] Duplicates retain their existing run; repository mismatch, stale generation, changed profile or revoked authority cannot admit new work. Previously accepted effects remain explicitly reconciled.
- GIVEN an enrolled connection and retained delivery
- WHEN authority changes or delivery repeats
- THEN stale admissions fail and duplicates retain their original identity.

### Requirement: Organization authority
r[kiln.hosted.accounts] Organization membership, role changes and invitations MUST be durable, scoped, attributable and revocable; cross-organization access and escalation MUST fail before data access or effects.

#### Scenario: Membership transition
r[kiln.hosted.accounts.scenario.membership] Invitation acceptance binds the intended peer and organization once; replay and stale membership cannot restore revoked authority or remove the final owner accidentally.
- GIVEN an invitation and current owners
- WHEN it is accepted or membership changes
- THEN peer binding, consume-once admission and final-owner protection hold.

### Requirement: Financial admission
r[kiln.hosted.finance] Hosted consumption MUST reserve a bounded maximum atomically before paid dispatch, bind exact integer price/class and attempt identities, durably deduplicate usage/invoice operations and preserve unknown obligations. Credits, invoices, payment and execution outcomes MUST remain distinct.

#### Scenario: Concurrent cap and uncertain effects
r[kiln.hosted.finance.scenario.cap] Competing requests cannot oversubscribe remaining spend. Restart and missing usage preserve reservations; duplicate receipts or invoice retries cannot double-charge.
- GIVEN a finite account cap
- WHEN concurrent requests reserve and restart
- THEN committed reservations remain within the cap and unknown usage remains reserved.

### Requirement: Customer pipeline authoring and migration
r[kiln.hosted.pipelines] Repository-owned typed pipeline configuration MUST compile deterministically with bounded templates, parameters, matrices and dependencies into reviewed execution requests. Migration MUST preserve supported semantics and reject unsupported behavior with actionable diagnostics before execution.

#### Scenario: Valid and unsupported configurations
r[kiln.hosted.pipelines.scenario.compile] A supported customer configuration compiles to a stable exact plan; cycles, expansion overflow, unknown actions/expressions and invalid fields are rejected without silently dropping jobs or authority requirements.
- GIVEN supported and unsupported configurations
- WHEN compilation or migration runs
- THEN supported semantics are deterministic and unsupported behavior produces errors before effects.

### Requirement: Isolated execution and elastic capacity
r[kiln.hosted.execution] Customer execution MUST use an explicitly admitted isolation backend with independently enforced resource/deadline policy, excluded platform credentials and owned cleanup. Fleet decisions MUST respect finite capacity, expose queue/cache timing observations and reconcile uncertain provisioning before retry.

#### Scenario: Failure and cleanup
r[kiln.hosted.execution.scenario.teardown] A real successful/failing/cancelled workload produces bounded observations and cleanup evidence; absent backend or uncertain teardown cannot become safe capacity or a hostile-isolation claim.
- GIVEN an admitted isolated backend
- WHEN workloads complete, fail or cancel
- THEN resource/cleanup observations remain bounded and uncertain teardown retains unavailable capacity.

### Requirement: Scoped secrets and workload identity
r[kiln.hosted.secrets] Secret delivery and short-lived workload identity MUST bind organization/project/job, audience, scope, expiry and current revocation. Provider credentials MUST remain outside customer authority except explicitly authorized job secrets. Tokens MUST preserve interoperability algorithms.

#### Scenario: Wrong audience and revocation
r[kiln.hosted.secrets.scenario.denial] Expired/replayed/revoked identity, cross-project secret requests and unapproved audiences are denied without exposing secret values in logs or public receipts.
- GIVEN a scoped secret or workload token
- WHEN audience, expiry or revocation differs
- THEN access fails without exposing values.

### Requirement: Hosted operations
r[kiln.hosted.operations] Hosted operation MUST provide bounded content retention separate from financial/replay retention, verifiable backup/restore, attributable incident admission holds and support observations. Restore MUST preserve exact authority, financial obligations and replay protection.

#### Scenario: Restore and incident hold
r[kiln.hosted.operations.scenario.restore] An actual restore reproduces admitted retained state and uncertain obligations; corrupt/incompatible backups fail closed and an incident hold denies new consumption without destroying ongoing recovery evidence.
- GIVEN durable state and an incident hold
- WHEN backup/restore and new admission run
- THEN valid recovery preserves obligations and the hold rejects new consumption.

### Requirement: Bounded qualification
r[kiln.hosted.qualification] Completion MUST retain observed integrated Iroh/browser and remote-source CI behavior plus positive/negative/recovery evidence for each changed boundary. No model, mock, loopback check or lifecycle gate alone establishes paid deployment, hostile isolation, availability, merchant or legal readiness.

#### Scenario: Missing production prerequisites
r[kiln.hosted.qualification.scenario.nonclaims] Missing operator/provider credentials, approved prices, merchant terms, production capacity or independent review remain explicit blockers; no fabricated live action or readiness claim is emitted.
- GIVEN missing production prerequisites
- WHEN qualification is reported
- THEN missing inputs remain blockers and no live outcome or approval is fabricated.

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
