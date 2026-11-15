## Context

`PausedHandoff` is serialized phase state. It must remain self-contained across persistence/reopen, and a method argument cannot establish continuity merely because its owner token is internally valid. `RetainedSession.identity` carries the immutable logical session, VM, work/revision, attempt/fence and execution identity; the source placement is captured separately. Checkpoint expectations are independently resolved by the caller and `admit_checkpoint` already compares every manifest binding against them.

## Decisions

### Decision: persist the original session identity and compare it on all commands

Add the full `SessionIdentity` to the handoff at `begin`. A shared identity-and-source check validates schema, invokes existing session/owner authorization, then compares the supplied session identity and source placement with the persisted origin. Every transition, including `record_source_retired` and `require_reconciliation`, uses this check. Serialized handoffs lacking the newly required identity fail closed (no inferred/default identity); no schema downgrade or alias path is added. Operation remains a distinct persisted handoff field and is checked against the manifest and cutover evidence.

Terminal source-retirement and reconciliation observations validate the exact owner token and immutable origin without requiring an active session. Ordered handoff commands additionally require the source session to remain active; a stale persisted intent cannot advance after source retirement.

### Decision: treat caller expectations as immutable evidence

`record_objects_verified` receives `CheckpointExpectation` by value but does not mutate it. Compare its session, VM, work/revision, attempt/fence/execution, source/target and operation fields to the persisted handoff origin, then pass it unchanged to `admit_checkpoint`. Disk/profile/generation remain independently resolved inputs and are validated by that admission function. Any refusal leaves both phase and manifest unchanged.

### Decision: bind target receipt and observations to exact handoff facts

`TargetAdmissionReceipt` pairs a native `ExternalRef` with its operation,
checkpoint identity, source/target placements, target session/fence and exact
cutover evidence. The phase transition validates every scope field against the
persisted handoff and verifies the manifest identity plus durable commit, source
volume/effect fences and destination acknowledgement. Resume and retirement
revalidate the scoped receipt, manifest and fencing evidence; resume also checks
the supplied target owner before advancing. A native reference is not proof of
provider truth; no local marker becomes distributed authority.

### Decision: cover every ordered boundary with refusal atomicity tests

Use the existing pure fixtures to test a complete happy path, independent fresh deserialization/reopen before later transitions, exact state snapshots on refused calls, substitutions that preserve host/placement but alter identity, each changed identity component, and wrong-operation/manifest/placement/fencing evidence. Exercise direct `admit_checkpoint` and wrapper admission with the same mismatch pairs.

## Risks / Trade-offs

- The serialized handoff record gains a required field; old handoff records are intentionally rejected rather than guessed. Callers must persist the new record shape before using subsequent commands.
- Exact origin binding prevents using a transitioned/replaced session object as an implicit proxy; shells must retain the original source identity while observing cleanup.
- Syntactic receipt validation and modeled evidence matching do not establish provider currentness, durable commit truth, or effective resource fencing; those remain provider responsibilities.
