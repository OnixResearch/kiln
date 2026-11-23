# Implementation Authority

## ADDED Requirements

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
maximum age, and when a proof in the chain is revoked. It MUST persist every
redacted decision, and an allowed decision that cannot be persisted MUST deny.

#### Scenario: Replayed invocation denies after restart

r[kiln.implementation.authority.state.scenario.replay]
- GIVEN a consumed holder invocation
- WHEN the same nonce is presented again by a restarted process
- THEN it MUST deny

#### Scenario: Stale revocation view denies

r[kiln.implementation.authority.state.scenario.stale]
- GIVEN a revocation view older than its configured maximum age
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
