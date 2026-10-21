## ADDED Requirements

### Requirement: Dedicated canary authority and bounded native topology

r[kiln.radicle_ci.live.authority] The live qualification MUST use a fresh private canary root, dedicated RID/branch, separately pinned source and status identities, exact executable/profile/workflow identities, declared native peers and finite process/log/deadline bounds. It MUST NOT use or mutate the operator's production node, repositories, signers, services, governance, visibility, mirrors or main refs. Only trusted reviewed canary source is admitted; host placement and network limits MUST remain explicit.

#### Scenario: Owned canary starts

- GIVEN a fresh root and an exact declared peer/RID/branch/signing cohort
- WHEN the canary provisions its native nodes and runtime
- THEN every effect MUST be confined to the declared canary state and identities.

#### Scenario: Authority or state is not fresh

- GIVEN pre-existing state, a foreign RID/branch/signer, an ambient production home or an undeclared peer
- WHEN canary admission runs
- THEN it MUST reject before source, signing or network effects.

### Requirement: Genuine peer delivery automatically invokes the broker

r[kiln.radicle_ci.live.ingress] The canary MUST run the actual pinned Radicle node and broker event subscription and deliver a genuine native peer source update. The broker MUST invoke the unchanged reviewed Kiln controller/worker workflow for the admitted exact revision without a manufactured protocol request, manual cibtool trigger, fixture provider result or per-event workflow rewrite. The native node event, original broker input and replies, exact signed source and canonical reservation MUST be retained.

#### Scenario: A remote-node revision arrives

- GIVEN a trusted admitted source revision and a running subscribed broker
- WHEN the native peer fetch/update completes
- THEN one canonical source-bound CI operation MUST be reserved and actual source bytes MUST reach the existing Nix worker automatically.

#### Scenario: No admitted peer event exists

- GIVEN missing or foreign peer events, incompatible producer data or a manually supplied protocol message
- WHEN the live criterion is evaluated
- THEN it MUST NOT be reported as genuine automatic remote ingress.

### Requirement: Real separate signer and independent status readback

r[kiln.radicle_ci.live.status] The canary MUST use the normal status service and pinned real SecretSpec executable with a separately declared canary status signer. After canonical terminal admission it MUST retain the reserved exact status intent and local write observation, explicitly sign/synchronize only the declared canary refs, and independently fetch and reduce the same status from another native storage instance. Repository, source revision, run, outcome, object ID and head MUST agree; broker-native job objects MUST NOT substitute for Kiln's projection.

#### Scenario: Exact status is independently visible

- GIVEN an admitted terminal canonical result and exact local dedicated status observation
- WHEN canary-only signing/synchronization and observer fetch complete
- THEN bounded read reconciliation in observer storage MUST return the same revision/run/outcome/object/head without new signing or a second write.

#### Scenario: Signer or remote status is wrong or absent

- GIVEN a foreign signer, wrong revision/head, missing real provider or absent independently fetched object
- WHEN status qualification runs
- THEN the live status criterion MUST fail and no fixture credential, broker projection or stale local observation MAY replace it.

### Requirement: Live duplicate and restart recovery preserve execution

r[kiln.radicle_ci.live.recovery] During actual native delivery the canary MUST exercise duplicate delivery/recovery and controller/host loss while the real worker independently executes. Canonical recovery MUST preserve the original reserved source, run/attempt and native operation, execute the worker once, and retain exact final status object/head. Upstream broker deduplication MUST be distinguished from controller reconciliation evidence; neither permits redispatch after uncertain acceptance.

#### Scenario: A running source event is recovered

- GIVEN one real native-delivered source event and an in-flight independent worker
- WHEN controller/host loss, restart and duplicate/recovery occur
- THEN canonical replay MUST retain the original operation and exactly one worker invocation through the final result and root handoff.

#### Scenario: A duplicate or restart cannot establish acceptance

- GIVEN a replayed event or interrupted dispatch without linked terminal evidence
- WHEN recovery observes the original operation
- THEN it MUST retain uncertainty rather than invent a new operation or successful completion.

### Requirement: Genuine failure and unavailable-provider uncertainty

r[kiln.radicle_ci.live.failure] The canary MUST deliver a source revision that genuinely fails its Nix build and a revision whose configured provider is unavailable. The failed build MUST produce matching canonical failure and independently read exact-revision failure status. The unavailable-provider case MUST retain bounded nonterminal uncertainty, no final broker reply/status and no worker redispatch on recovery; a process failure MUST NOT manufacture a canonical build failure.

#### Scenario: Source actually breaks the build

- GIVEN an admitted remote source revision containing the reviewed failure control
- WHEN the real Nix build runs
- THEN native diagnostics, canonical failure and independently fetched revision-bound failure status MUST agree.

#### Scenario: Provider cannot establish an outcome

- GIVEN an admitted remote source revision with its declared provider unavailable
- WHEN finite observation and duplicate/recovery complete
- THEN the canonical journal MUST remain nonterminal with the original operation and no final status or fabricated broker Finished result.

### Requirement: Repeatable live evidence and bounded claims

r[kiln.radicle_ci.live.evidence] Kiln maintainers MUST retain fresh-state repeat commands, exact source/executable/profile/workflow and native topology identities, actual peer/broker ingress, positive and negative native observations, canonical original-operation journals, Nix products/diagnostics, independently fetched status, explicit root discharge, rejected attempts and private credential/process retirement. They MUST run required affected and repository gates and synchronize/archive only after actual prerequisites pass. Same-host native TCP MUST NOT be promoted to WAN, distinct-host isolation, worker confinement, production rollout, activation or release approval.

#### Scenario: Complete native canary repeats

- GIVEN the recorded exact cohort and fresh dedicated state
- WHEN all declared live cases and required gates execute
- THEN retained evidence MUST reproduce the checked automatic peer-to-CI-to-status boundary without private key/seed disclosure.

#### Scenario: Live evidence or authority is unavailable

- GIVEN an unavailable genuine node/broker/provider/observer prerequisite or missing mutation authority
- WHEN the qualification is evaluated
- THEN the missing criterion MUST remain explicit; fixtures, old receipts or lifecycle task completion MUST NOT be promoted to the absent proof.
