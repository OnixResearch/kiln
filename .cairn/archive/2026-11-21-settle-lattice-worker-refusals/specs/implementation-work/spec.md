## ADDED Requirements

### Requirement: Lattice implementation CLI coordinator transport

r[kiln.implementation.lattice_cli] The worker transport to `lattice implementation-attempt` MUST bind the reserved dispatch-plan identity into the exact request bytes, MUST pass the Lattice process no ambient environment except the one credential variable the admitted request names, MUST recover only with the Lattice program and state root bound at the attempt's first launch, and MUST settle an attempt as ended before any effect only when the first `run` or a bound recovery proves that no worker effect of that attempt can have started. Every other refusal or unclassified outcome MUST remain a possible dispatch for reconciliation. The Nickel transport configuration and campaign binding MUST admit only the transport generation the campaign requires; the direct worker runtime MAY keep its pre-existing v1 branch.

#### Scenario: Request bytes bind the reserved plan

r[kiln.implementation.lattice_cli.scenario.plan]
- GIVEN a campaign attempt whose native request is prepared after its plan digest is known
- WHEN Kiln binds the request and dispatches it
- THEN the prepared request MUST carry `dispatch_plan` equal to the reserved plan digest with an unchanged worker profile, and a request whose `dispatch_plan` is absent or different, or an observation echoing another plan, MUST be refused before transport intent or reconciliation

#### Scenario: Exactly one credential variable reaches Lattice

r[kiln.implementation.lattice_cli.scenario.credential]
- GIVEN an admitted request whose model broker names an `env` credential, a malformed or non-`env` credential, a reserved process-control name, or no credential
- WHEN Kiln launches `run`
- THEN only the named variable MUST be passed and every other variable withheld, a missing or empty value, a malformed reference or key and a reserved name MUST be refused after claim authorization and before plan reservation, request sealing, admission, the transport binding or the transport intent, and no declared credential MUST keep a fully cleared environment

#### Scenario: Recovery uses the root bound at the first launch

r[kiln.implementation.lattice_cli.scenario.bound_root]
- GIVEN an attempt whose first `run` was launched with one Lattice program and state root, and a later profile naming another root
- WHEN Kiln recovers the attempt
- THEN the binding MUST have been durably retained before the transport intent and recovery MUST use the bound program and root, and an absent binding, a binding of another attempt or fence, a conflicting rebinding or a bound root that is not a directory MUST keep the unknown outcome without invoking Lattice

#### Scenario: Only a before-effect refusal of the first run settles the attempt

r[kiln.implementation.lattice_cli.scenario.refusal]
- GIVEN a non-zero Lattice exit that prints a `lattice.implementation-attempt-refusal.v1` record for each reason in Lattice's table, from `run` or from `recover`
- WHEN Kiln classifies it
- THEN only a `run` record that is exactly Lattice's refusal for the request's work, attempt and fence with `dispatched:false` and reason `crossed_attempt`, `profile_revision_superseded`, `revision_not_admitted` or `revision_blocked` MUST end the attempt as an infrastructure failure without an unknown outcome or recorded execution, while `stale_fence`, every `recover` refusal, `dispatched:true`, a reason Lattice always marks as dispatched, an unknown reason, a crossed record, duplicate or unknown fields such as a result or artifacts, a misplaced or missing `revision_refusal`, malformed output, a timeout or a cancellation MUST keep the attempt for reconciliation

#### Scenario: Recovery closure is not an execution

r[kiln.implementation.lattice_cli.scenario.closed]
- GIVEN bound recovery that reports `not_started_closed_by_recovery` for the exact envelope, request identity and plan
- WHEN Kiln records the observation
- THEN the attempt MUST end with no recorded workflow execution and no unknown outcome, and a closure for another envelope, request identity or plan, one with duplicate or unknown fields, a result, acceptance or integration, or a closure reported by `run` MUST be refused

#### Scenario: Transport configuration admits generation v2

r[kiln.implementation.lattice_cli.scenario.config]
- GIVEN Nickel transport configuration naming `kiln.lattice-implementation-cli.v2` or `v1`
- WHEN it is exported
- THEN v2 MUST export and v1 MUST be rejected
