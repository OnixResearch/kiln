## ADDED Requirements

### Requirement: One-shot exact event source admission and materialization

r[kiln.radicle_ci.source] The composition MUST admit only the configured repository, signer namespace, branch and supported push event. It MUST capture one exact signed-refs identity and the event revision, verify the existing complete local source contract, and materialize/freeze only that verified source under declared bounds. Recovery MUST retain the reserved exact identities rather than follow a moved branch. Unsafe paths, symlinks, gitlinks, missing or stale identities and exceeded bounds MUST fail before workflow effects.

#### Scenario: Exact admitted event

- GIVEN a configured repository/branch and an actual event whose exact revision is verified by its signed refs
- WHEN source acquisition/materialization completes
- THEN retained receipt and worker source MUST identify those exact bytes and revision.

#### Scenario: Wrong or unsafe source

- GIVEN a foreign repository/branch, stale signed refs, changed revision or unsupported tree member
- WHEN source admission runs
- THEN no workflow operation MUST be submitted and no alternate source MAY be selected.

### Requirement: Fixed reviewed workflow builds its acquired source

r[kiln.radicle_ci.runtime] A fixed reviewed project profile and workflow operation MUST receive the admitted source context, acquire/freeze that exact source and execute the existing bounded Nix discovery/build adapters. The worker MUST retain immutable source proof, selected task/build identities, actual native outputs/diagnostics and existing rooted-result handoff/release. A separately supplied or edited consumer snapshot, synthetic provider or per-event operator signature MUST NOT substitute for the event source or real build.

#### Scenario: A new exact revision is built

- GIVEN the static reviewed project/workflow and a new admitted exact revision
- WHEN its mapped real worker executes
- THEN the result MUST bind the same project profile, source receipt, frozen source, discovery/build observations and exact rooted outputs without rewriting the workflow operation.

#### Scenario: Worker source context drifts

- GIVEN a changed source context, source/profile identity or missing native input
- WHEN worker admission or recovery runs
- THEN it MUST reject the drift without fallback or a successful build claim.

### Requirement: Canonical source-bound CI journal precedes dispatch

r[kiln.radicle_ci.journal] The broker-driven composition MUST persist canonical StartRun intent and exact native ingress before dispatch using existing pure CI transitions. Terminal CI completion MUST require linked actual source/worker and native-host observations, selected discovered task and admitted build result. Journal replay MUST reconstruct the source-bound canonical outcome; broker replies, process exits and status projections alone MUST NOT establish it.

#### Scenario: Real completion is admitted

- GIVEN exact matching native terminal and actual source/worker result
- WHEN completion admission runs
- THEN canonical replay MUST expose the matching run/attempt outcome and products.

#### Scenario: Evidence is absent or substituted

- GIVEN missing, conflicting or substituted source/native/worker observations
- WHEN reconciliation runs
- THEN success MUST NOT be synthesized and no final success status MAY be published.

### Requirement: Duplicates and restart preserve the original execution

r[kiln.radicle_ci.recovery] Duplicate delivery and controller/host restart MUST retain the original reserved trigger, source/profile identities, run/attempt and native operation. Recovery MUST reconcile without normal redispatch. Unknown acceptance, interrupted worker intent and bounded polling exhaustion MUST remain recoverable uncertainty rather than authorize a new execution.

#### Scenario: Restart during actual build

- GIVEN one reserved operation and an independently continuing real worker
- WHEN the controller/host restarts and the event is delivered again
- THEN evidence MUST show the same operation and one actual invocation with exact recovered outcome/root handoff.

#### Scenario: Acceptance cannot be proven

- GIVEN an interrupted or unknown dispatch with no linked terminal observation
- WHEN recovery reaches its finite observation bound
- THEN the journal MUST retain uncertainty without redispatch or final successful status.

### Requirement: Dedicated exact-revision status intent and reconciliation

r[kiln.radicle_ci.status] The dedicated versioned status contract MUST bind repository, exact source revision, canonical run, idempotency key and terminal outcome in its plan, persisted COB action and write observation. Profile authority MUST remain separate from trigger/source authority. The controller MUST durably reserve the exact status intent before publication and reconcile an interrupted/unknown write without blind redispatch. Pending/unknown canonical outcomes, mismatched revisions and incompatible historical actions MUST NOT become valid final status.

#### Scenario: Terminal source-bound status repeats

- GIVEN an admitted canonical terminal result and separately authorized status profile
- WHEN publication and exact reconciliation repeat
- THEN the same revision/run/outcome and object/head MUST be retained without an extra logical status.

#### Scenario: Revision or publication truth is uncertain

- GIVEN a reused key with a different revision or an interrupted write without exact observed status
- WHEN status admission/reconciliation runs
- THEN the conflict MUST fail closed or remain uncertain, with no fabricated successful publication and no implicit remote synchronization.

### Requirement: Actual repeatable evidence preserves operator authority

r[kiln.radicle_ci.evidence] Kiln maintainers MUST retain typed configuration, exact source/executable/profile identities, actual producer/runtime/source/build/status commands and positive/negative observations, original-operation duplicate/restart proof, rooted handoff/release, credential/process retirement and required native gates. Synthetic protocol fixtures MUST remain distinguished from actual producer execution. The change MUST NOT authorize production initialization/signing, push, synchronization, announcement, governance or mirror mutation, activation, deployment or release.

#### Scenario: Bounded native composition repeats

- GIVEN the declared source/producer/runtime cohort and isolated owned state
- WHEN success, genuine failure, duplicate and restart cases execute
- THEN their exact source, canonical result, status and retirement observations MUST be retained with repeatable commands.

#### Scenario: A required live prerequisite is unavailable

- GIVEN unavailable genuine producer evidence or missing operator mutation authority
- WHEN delivery evidence is evaluated
- THEN the unmet criterion MUST remain explicit and fixtures or lifecycle metadata MUST NOT be promoted to that proof.
