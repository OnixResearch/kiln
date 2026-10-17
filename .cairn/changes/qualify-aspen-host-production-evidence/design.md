## Context

The accepted Aspen-host evidence boundary requires real-process durability, recovery, corruption, provider teardown, finite load, backup/restore, explicit rollback, authority and exact publication linkage. Current global coverage is 622/625. The standalone host already owns durable SQLite/Redb/value state and exact Lattice reconciliation; its pinned producer cohort is independent of root-workspace imports.

## Decisions

### Decision: Preserve the accepted meaning and exact cohort

**Choice:** Classify this as implementation and qualification of existing requirements, with no normative spec delta. Inventory old receipts before running cases and bind fresh observations to actual source, executable, runtime/provider profile and artifact identities.

**Rationale:** A passing old check or a moving source ref cannot qualify changed inputs. Historical source-publication exceptions remain historical and cannot waive production evidence.

### Decision: Keep admission pure and execution isolated

**Choice:** Reuse the native host/provider fixtures. Put complete-cohort, required-case/gate and bounded-claim decisions in a pure host qualification boundary; keep file observation, hashing, process control and receipt I/O in thin shells. Retain positive and negative executable cases together.

**Rationale:** Receipt admission can prove only checked consistency of supplied observations. Actual process results and artifact bytes must be collected separately, and no accepted receipt supplies deployment or publication authority.

### Decision: Extend existing mechanisms only where evidence is missing

**Choice:** Reuse valid restart/corruption/admission tests and add discriminating real-process load, backup/restore, rollback or teardown coverage only where absent. Exercise temporary roots and loopback transports, not deployed operator services. Keep deadlines, request counts and teardown limits named and finite.

**Rationale:** This builds repeatable qualification for the existing CI consumer without duplicating the host, introducing a new executor, or adding unrelated runtime features.

## Risks / Trade-offs

- Local process recovery and synchronized database/filesystem observations do not prove device persistence, global durability, distributed exactly-once delivery or provider truth.
- A finite load observation is not a production capacity, soak or availability guarantee; retain its exact workload and bounds.
- Backup/restore and explicit rollback must retain the complete matching state/profile/executable cohort; a partial or mismatched restore must fail closed.
- Required broad gates must pass for the recorded input cohort. A failed gate remains a failure; evidence-only bookkeeping is not retroactively included in its own receipt.
- Qualification does not authorize live service changes, secret disclosure, remote status writes, publication, governance changes or lifecycle archival.

