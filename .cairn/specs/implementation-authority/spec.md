# Implementation Authority Specification

## Purpose

Defines the `implementation-authority` capability.

## Requirements

### Requirement: Closed protected-operation coverage

r[kiln.implementation.authority.coverage] Kiln MUST retain a closed coverage map
from every protected path in the selected implementation profile to exactly one
reviewed Basalt operation, one enforcement owner, one effect class and whether a
candidate identity is bound. An operation or protected path without an entry MUST
deny, and no operation MUST imply another.

#### Scenario: Every protected operation is mapped once

r[kiln.implementation.authority.coverage.scenario.complete]
- GIVEN the closed claim, renewal, dispatch, submission, verification, cancellation, publication, integration and lifecycle operations
- WHEN the coverage map is inspected
- THEN every operation MUST appear exactly once with a unique owner and the reviewed candidate rule
- AND the map MUST contain no duplicate Basalt operation id

#### Scenario: Unmapped path denies

r[kiln.implementation.authority.coverage.scenario.unmapped]
- GIVEN a protected path with no coverage entry
- WHEN authority is requested for it
- THEN the request MUST deny before any effect

### Requirement: Exact request binding

r[kiln.implementation.authority.binding] Each authority request MUST bind the
holder, deployment audience, project, work and version, attempt and fence,
authority generation, canonical payload digest, target and the applicable
candidate identity. The caller MUST NOT supply a trusted current-state
observation, and substituting any bound field MUST invalidate the request.

#### Scenario: Substituted binding denies

r[kiln.implementation.authority.binding.scenario.substitution]
- GIVEN a verified request for one exact operation
- WHEN any bound field differs
- THEN authorization MUST deny before credential resolution or effect

#### Scenario: Candidate is bound exactly where a mutation needs one

r[kiln.implementation.authority.binding.scenario.candidate]
- GIVEN submission, verification, publication, integration and lifecycle bind a candidate
- WHEN the candidate identity is absent for one of them, or present for claim, dispatch or cancellation
- THEN authorization MUST deny

### Requirement: Verified live authority before protected effects

r[kiln.implementation.authority.enforcement] A protected read, disclosure,
mutation or external effect MUST require current verified authority for that exact
request. A caller boolean, role label, serialized receipt, provider capability
advertisement or historical approval MUST NOT manufacture authority. Credentials
MUST be resolved only after authorization succeeds.

#### Scenario: Missing authority denies before effect

r[kiln.implementation.authority.enforcement.scenario.absent]
- GIVEN a claim, dispatch, submission, verification, publication, integration or lifecycle request without a verified session
- WHEN the owning boundary evaluates it
- THEN it MUST deny before any mutation, provider call or credential resolution

#### Scenario: Retained allow evidence is not authority

r[kiln.implementation.authority.enforcement.scenario.replay]
- GIVEN a serialized request, redacted receipt or prior allowed decision
- WHEN it is presented for a new effect
- THEN it MUST NOT authorize that effect

### Requirement: Currentness blocks superseded attempts

r[kiln.implementation.authority.currentness] Revalidation MUST check signatures,
proofs, time, revocations, policy and deployment generation before each protected
effect without consuming replay twice. Cancellation, supersession, lease expiry
and fence replacement MUST block later effects for the affected attempt even when
another still-valid proof is presented. Unavailable required currentness MUST fail
closed for new effects; independently authorized reconciliation and cleanup MUST
remain possible.

#### Scenario: Revoked or replaced attempt is blocked

r[kiln.implementation.authority.currentness.scenario.blocked]
- GIVEN a cancelled, superseded or revoked attempt
- WHEN a later effect is requested with any proof
- THEN the effect MUST deny
- AND retained evidence and authorized cleanup MUST remain available

#### Scenario: Unavailable currentness fails closed

r[kiln.implementation.authority.currentness.scenario.unavailable]
- GIVEN a required revocation or authority source that cannot be read
- WHEN a new effect is requested
- THEN it MUST deny rather than assume current authority

### Requirement: Execution authority is not verification or publication authority

r[kiln.implementation.authority.separation] Claim, dispatch and candidate
submission authority MUST NOT admit verification acceptance, publication,
integration or native lifecycle completion. Each protected operation MUST be
authorized by its own grant, and an authorization decision MUST NOT create a
passing check or discharge a Cairn obligation.

#### Scenario: Dispatch grant cannot publish

r[kiln.implementation.authority.separation.scenario.widen]
- GIVEN a session admitted for dispatch or submission
- WHEN publication, integration or lifecycle completion is requested
- THEN it MUST deny

### Requirement: Explicit local-operator compatibility

r[kiln.implementation.authority.compatibility] A local-operator compatibility
profile MAY exist only when explicitly selected and MUST record its provenance in
the decision. It MUST NOT authorize projection-enabled routes, MUST NOT widen by
role alone, and MUST NOT provide a fallback for missing authority.

#### Scenario: Role label alone denies

r[kiln.implementation.authority.compatibility.scenario.role]
- GIVEN an operator or worker role label without a verified session or the selected local-operator profile
- WHEN a protected operation is requested
- THEN it MUST deny

### Requirement: One UCAN cohort and recorded Basalt import

r[kiln.implementation.authority.cohort] Kiln MUST serve observation authentication
and implementation authority from one vendored UCAN snapshot and MUST import the
Basalt adapter through a reproducible repository-owned transform that records the
source revision, archive identity and path rewrites.

#### Scenario: Import is reproducible

r[kiln.implementation.authority.cohort.scenario.import]
- GIVEN the recorded Basalt import
- WHEN the import script runs again
- THEN it MUST produce the same vendored tree
- AND the manifest rewrite MUST be recorded, not inferred

### Requirement: Reviewed entry-point classification

r[kiln.implementation.authority.coverage.entry_points] Kiln MUST classify every
`Implementer` public function, implementer CLI subcommand, fleet API route and
web campaign control route as `Protected(op)`, `Reconcile(op)`,
`ObservationOnly` or `OperatorConfig`. Every protected operation MUST have a
`Protected` entry at its coverage owner path. A `Protected(op)` entry MUST reach
the `op` authority check before its effect. A non-authority entry MUST NOT check
authority that its rows do not name.

#### Scenario: Unlisted entry point fails coverage

r[kiln.implementation.authority.coverage.entry_points.scenario.unlisted]
- GIVEN a new public entry point with no reviewed row, or a row whose entry point no longer exists
- WHEN the source-scanning coverage test runs
- THEN it MUST fail and name the entry point

#### Scenario: Every claim path admits ClaimAcquire

r[kiln.implementation.authority.coverage.entry_points.scenario.claim]
- GIVEN a ledger whose selected profile denies the claim, or no selected profile
- WHEN work is claimed through `claim_next` or `claim_next_deferred`
- THEN the claim MUST deny inside the claim transaction
- AND no attempt MUST be committed

### Requirement: Dispatch admitted before reservation, launch and credentials

r[kiln.implementation.authority.dispatch] Every dispatch reservation
(`reserve_dispatch`, `reserve_dispatch_plan`), every durable launch record
(`start_execution`, the fleet session bind) and every fleet launch effect MUST
admit the exact `Dispatch` request for the attempt, fence and reservation
effect identity before it saves a record or starts an effect. The decision MUST
run inside the transaction that would save the record, after the attempt
ledger's fence and phase checks. A shell that reads a credential before
reserving MUST first admit the same request without changing state, and the
reservation MUST then revalidate that session rather than admit a second
grant. The transport-time recheck MUST remain.

#### Scenario: Admitted principal reserves and launches

r[kiln.implementation.authority.dispatch.scenario.admitted]
- GIVEN a claimed attempt under the verified profile or the local operator
- WHEN it is reserved, bound to a fleet session, rechecked before launch and started
- THEN each step MUST admit
- AND the pre-check and the reservation MUST consume one invocation together

#### Scenario: Denied dispatch commits and starts nothing

r[kiln.implementation.authority.dispatch.scenario.denied]
- GIVEN the absent profile, a revoked grant, a stale revocation view, or a wrong attempt or fence
- WHEN the fleet launch, `reserve`, `reserve-plan`, the pre-check or the CLI `start` runs
- THEN it MUST deny
- AND no reservation, plan, session or execution MUST be recorded
- AND no invocation MUST be consumed

#### Scenario: Denial precedes the credential read

r[kiln.implementation.authority.dispatch.scenario.credential]
- GIVEN a worker run whose admitted request names one credential variable
- WHEN its Dispatch pre-check denies
- THEN the credential source MUST NOT be read
- AND no reservation MUST be recorded

### Requirement: Canonical payload binding

r[kiln.implementation.authority.payload] The request digest MUST be the
length-framed BLAKE3 of the operation, the worker incarnation (the attempt's
recorded transport launch, or `pre-launch`), the stable effect identity, the
expected remote target state and the exact payload parts. The receiving owner
MUST recompute it from its own facts.

#### Scenario: Every payload field changes the digest

r[kiln.implementation.authority.payload.scenario.fields]
- GIVEN one protected payload
- WHEN the operation, incarnation, effect identity, expected target or any part changes, or parts are re-split
- THEN the digest MUST change
- AND recomputing the same payload MUST produce the same digest

### Requirement: Configured profile selection

r[kiln.implementation.authority.config] Each shell MUST select its authority
profile from an explicit local operator or an exported
`kiln.implementation-authority-config.v1`, never both. Absence of either MUST
select the absent profile, which denies. Invalid configuration, and a role
holder without exactly one current grant, MUST refuse selection.

#### Scenario: Expired or untrusted grants are not selected

r[kiln.implementation.authority.config.scenario.refused]
- GIVEN a verified configuration whose grants are expired or issued by an untrusted root
- WHEN the profile is selected
- THEN selection MUST fail
- AND the ledger MUST keep the absent profile and deny claims

#### Scenario: Campaign roles select the drive's profile

r[kiln.implementation.authority.config.scenario.roles]
- GIVEN a campaign drive that selected a local operator, an exported configuration or neither
- WHEN it launches its worker, capture or verifier role
- THEN the role MUST receive exactly that source and resolve the same profile
- AND an admitted worker MUST reach its transport check while the absent profile is refused before any credential read or reservation

### Requirement: Role-separated holder signing

r[kiln.implementation.authority.selection] The verified profile MUST sign each
holder invocation with the key of the role that owns the operation. The
coordinator, verifier and publisher keys are distinct. Each entry point MUST
load only the holder seeds of the roles its own protected operations need,
fixed by the entry point and never widened by a flag, and MUST refuse an
operation of any other role before it signs, records or uses a credential.
The local-operator profile MUST carry no signer.

#### Scenario: A role key cannot sign another role's operation

r[kiln.implementation.authority.selection.scenario.roles]
- GIVEN a coordinator, verifier or publisher holder key
- WHEN it signs an operation owned by another role
- THEN verification MUST deny

#### Scenario: An entry point loads only its own holder seeds

r[kiln.implementation.authority.selection.scenario.scoped]
- GIVEN the worker or capture role with the verifier and publisher seed files absent or unreadable, or the verifier with only its own seed
- WHEN the role selects the verified profile and runs its own operations
- THEN selection MUST succeed without opening the other seeds and its operations MUST admit
- AND an operation of another role MUST be refused as holder-not-loaded, consuming no invocation and recording nothing

### Requirement: Durable replay, revocation and decisions

r[kiln.implementation.authority.state] The verified profile MUST consume each
holder invocation once in a durable replay table in its own immediate
transaction. It MUST deny when the revocation view is absent or older than its
maximum age, and when a proof in the chain is revoked. The view's age MUST be
measured from the observation time of the authority's own configured
revocation feed; importing a feed MUST NOT refresh it, and another feed
sharing the same durable state MUST NOT count. It MUST persist every redacted
decision, and an allowed decision that cannot be persisted MUST deny.

#### Scenario: Replayed invocation denies after restart

r[kiln.implementation.authority.state.scenario.replay]
- GIVEN a consumed holder invocation
- WHEN the same nonce is presented again by a restarted process
- THEN it MUST deny

#### Scenario: Stale revocation view denies

r[kiln.implementation.authority.state.scenario.stale]
- GIVEN a revocation feed observed longer ago than its configured maximum age, however recently a process imported it or another feed sharing its durable state was imported
- WHEN a protected operation is requested with a grant that is otherwise valid
- THEN it MUST deny and commit nothing

#### Scenario: Revoked grant denies

r[kiln.implementation.authority.state.scenario.revoked]
- GIVEN a grant listed in the configured revocation feed
- WHEN its holder requests a protected operation
- THEN it MUST deny, consume no invocation and commit nothing

### Requirement: Fence checked at commit

r[kiln.implementation.authority.interleaving] An authority check MUST NOT stand
in for the fence check at commit. A fence change committed after the check MUST
refuse the effect, and no writer MUST change the fence inside the transaction
that applies an authorized mutation.

#### Scenario: Fence changes after the check

r[kiln.implementation.authority.interleaving.scenario.after_check]
- GIVEN a Dispatch request admitted and revalidated
- WHEN another connection replaces the attempt before the transport transaction
- THEN transport MUST be refused as stale
- AND the owner reservation MUST remain committed, not unknown

#### Scenario: Fence cannot change inside the transaction

r[kiln.implementation.authority.interleaving.scenario.inside_transaction]
- GIVEN the Dispatch check inside the admission transaction
- WHEN a second connection tries to take the writer lock
- THEN it MUST fail as locked

### Requirement: Reconciliation reuses the effect identity

r[kiln.implementation.authority.reconcile] Adopting an observed outcome of an
admitted effect MUST present a fresh invocation bound to the original effect
identity. It MUST NOT mint a new effect identity.

#### Scenario: Lost response is reconciled with a fresh nonce

r[kiln.implementation.authority.reconcile.scenario.fresh_nonce]
- GIVEN a dispatch whose response was lost
- WHEN a restarted process reconciles the observed run
- THEN the original nonce MUST deny
- AND a fresh nonce over the original reservation's effect identity MUST admit
