## ADDED Requirements

### Requirement: Real combined CI composition

r[kiln.live_ci.composition] The maintained live CI exercise MUST compose ordinary GitHub, exact-source job, Iroh, and reviewed Lattice runtime interfaces on two distinct hosts. It MUST bind the exact forge repository identity, source revision, original exported profile bytes, native graph/workflow/operation identities, admitted worker peer, execution identity, canonical Kiln outcome, and real GitHub status. Fixture transports, fabricated signed events, a Pending attempt, or a marker-only command MUST NOT satisfy the exercise.

#### Scenario: Genuine event completes remotely

- GIVEN an authorized GitHub repository, signed forge delivery, reviewed compile/test command, enrolled separate worker, and exact runtime profiles
- WHEN ordinary webhook intake dispatches through Iroh and the worker's Lattice runtime completes the exact-source command
- THEN evidence MUST join that delivery and source to the worker execution, admitted terminal outcome, and matching Published real commit status.

#### Scenario: A fixture or incomplete result is offered

- GIVEN a synthetic event, local source/API fixture, same-host worker, Pending attempt, or unmatched remote status
- WHEN the live exercise is evaluated
- THEN it MUST NOT report combined live CI completion.

### Requirement: Real passing and failing tests

r[kiln.live_ci.outcomes] The exercise MUST compile and execute reviewed behavioral assertions against exact acquired repository content. It MUST observe a passing test as canonical Success and GitHub success, and a genuinely failing assertion/nonzero test exit as canonical DomainFailure and GitHub failure. It MUST check both local Published status intent and exact remote status identity.

#### Scenario: Passing and failing revisions execute

- GIVEN reviewed positive and negative source inputs and the exact bound workflow
- WHEN real tests run on the worker
- THEN each run MUST retain its actual matching canonical and remote terminal result without fallback or suppressed failure.

#### Scenario: Publication does not match local outcome

- GIVEN a terminal local CI result but missing, failed, unknown, stale, or conflicting remote status evidence
- WHEN completion is assessed
- THEN the exercise MUST report incomplete publication rather than claiming end-to-end success.

### Requirement: Interrupted controller recovery

r[kiln.live_ci.recovery] After remote acceptance, the exercise MUST interrupt and restart only its own controller and Iroh bridges while leaving Lattice and its job running. It MUST preserve protected keys, profiles, SQLite and Lattice state, recover the original execution through native recovery, reject duplicate work, and obtain the matching terminal GitHub status. It MUST NOT claim resumed execution after killing Lattice or blindly redispatch uncertain work.

#### Scenario: Controller and bridges restart during a job

- GIVEN a durably accepted still-running remote execution
- WHEN the controller and both transport bridges restart with unchanged identity/state
- THEN the same execution MUST converge to an admitted terminal outcome and Published real GitHub status without a second job execution.

#### Scenario: Executor itself is lost

- GIVEN an interrupted nonterminal Lattice executor
- WHEN only an orphaned uncertain native record remains
- THEN evidence MUST retain Unknown and MUST NOT claim controller-only recovery or exactly-once effects.

### Requirement: Bounded operator authority and repetition

r[kiln.live_ci.authority] The composition MUST use real pinned SecretSpec resolution, protected private state, explicit peer enrollment, and operator-reviewed workload/forge authority. Source and status credentials MUST remain inaccessible to executed repository commands. Its isolated validation resources MUST NOT mutate main, governance, existing CI/services, or unrelated ingress. Evidence and instructions MUST name maintenance ownership, exact reproduction inputs, temporary credential scope exceptions/retirement when applicable, and non-claims for confinement, WAN/relay availability, indefinite deployment, and release authority.

#### Scenario: Authorized validation is repeated

- GIVEN fresh private roots, reviewed immutable inputs, enrolled hosts, and authorized isolated forge/ingress resources
- WHEN the documented composition is repeated
- THEN it MUST exercise the same contracts without secret-bearing evidence or an ambient credential fallback.

#### Scenario: Existing authority or unrelated service would be changed

- GIVEN an existing main ref, production state root, CI service, or ingress handler outside the validation resource set
- WHEN deployment or cleanup considers that resource
- THEN it MUST remain unchanged without separate operator authority.
