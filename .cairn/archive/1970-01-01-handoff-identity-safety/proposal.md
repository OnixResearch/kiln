## Why

The paused-handoff record currently binds only operation and placements. Later commands validate a supplied session against its supplied owner token, not against the session that began the operation; reopening serialized state therefore leaves caller substitution possible. Object verification also rewrites independently supplied checkpoint expectations from the session, hiding mismatches. Target admission evidence must remain bound to the exact originating operation and checkpoint.

## What Changes

- Persist the originating retained-session identity in each paused handoff and require exact equality for every phase command, including retirement and reconciliation after deserialize/reopen.
- Preserve independently resolved checkpoint expectations and reject any mismatch rather than normalizing fields from current session or handoff state.
- Bind target admission, resume and retirement observations to the original operation, complete manifest, source/target placement, owner identity, and exact cutover/fencing evidence. Replace the bare `ExternalRef` admission input with a serialized scoped `TargetAdmissionReceipt`; migrate every current caller and reject unrelated or stale references without advancing phase.
- Add healthy and negative phase/reopen regressions, including caller substitution, changed attempt/fence/execution/revision/placement, crossed operations, independent manifest facts, and forged target evidence.

## Impact

- **Consumer/outcome:** pure paused-handoff callers must supply the original independent expectation unchanged; mismatched identity or evidence is refused without phase advancement.
- **Owner:** Kiln microVM handoff contract and its documented pure boundary.
- **Dependencies/API:** no provider, VMM, storage, transport or authority dependency. The pure `record_target_admitted` API now accepts `TargetAdmissionReceipt` instead of an unscoped `ExternalRef` plus separate evidence; migrate all in-repository callers.
- **Verification:** focused handoff regressions plus native proposal/design/tasks and applicable Cairn traceability/release gates, as performed by the integration owner.
- **Non-claims:** no source/VMM effects, distributed ownership, writer fencing, external-effect fencing, native migration qualification, or provider receipt truth is established by the pure contract.
