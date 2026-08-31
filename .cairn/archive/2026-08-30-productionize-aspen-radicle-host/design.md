## Context

The reviewed host at `69c0a6ac454d7291e4aed12fd72a6f2c31636e76` composes the real Aspen native service and Lattice exchange. It intentionally retains acknowledgements, uncertainty, trigger links, callback values, and the Aspen journal in memory. Its documented rollout gate denies production until durable restart, corruption, teardown, load, backup, rollback, authority, and operator evidence exist.

The current Onix broker also needs a bounded build effect. The canary workflow only validates source environment variables and prints a marker. A production promotion that changes only the broker command would therefore report fixture success instead of running CI.

## Search contract

**Goal:** publish a Kiln cohort that can host real Radicle build effects through Aspen and recover exact hosted operations after process loss.

**Completion evidence:** deterministic positive and negative tests, a real child process, a real Lattice exchange, durable restart and corruption fixtures, bounded report bytes, no provider redispatch after uncertain acceptance, strict Octet, Nix package checks, and archived Cairn evidence.

**False completion:** a model service, recording port, fixture shell, direct broker-side Nix process, process-local replay, fallback to direct Lattice, successful command without a retained report, or task completion without deterministic receipts.

**Audit risks:** duplicate external effects, stale operation substitution, state corruption, callback value loss, report path escape, unbounded output, child-process leakage, hidden environment authority, and self-referential publication.

**Budget:** four mechanism families, two search rounds, repository sources plus live Onix observations, and deterministic validators as final authority.

## Approach registry

| Family | Mechanism | State | Evidence or blocker |
|---|---|---|---|
| Direct canary promotion | Point the broker at the current Aspen socket | Falsified | The workflow is a marker fixture and host operation state is process-local. |
| Broker-side compatibility wrapper | Run the old Nix executor before or beside Aspen | Falsified | The protected build effect bypasses Lattice and does not prove hosted execution. |
| Merge the old direct executor | Restore `std::process::Command` inside the broker adapter | Falsified | It regresses the application-port boundary and still lacks durable hosted recovery. |
| Durable hosted provider | Persist exact host state, run a bounded Kiln provider under Lattice, and reconcile after restart | Selected | It preserves owner boundaries and has discriminating restart, corruption, and duplicate-effect tests. |

The passes are serial and correlated because no subagent consent was granted.

## Decisions

### Decision: Keep durable meaning in Kiln and persistence in the host shell

**Choice:** Add a Kiln-owned durable operation ledger. Use Aspen's durable native journal and a capability-rooted callback value adapter at the host composition root.

**Rationale:** Kiln owns replay, uncertainty, terminal meaning, and safe redispatch decisions. Aspen owns the generic journal and callback materialization contracts. Filesystem and database effects remain outside both cores.

### Decision: Recover exact operations instead of restarting them

**Choice:** Persist ingress before callback execution. Mark a possible provider write as uncertain. On repeated submit or reconcile, use the exact Lattice recovery operation and never call dispatch again.

**Rationale:** A crash boundary cannot turn uncertainty into a safe retry. Recovery must bind the original operation, trigger, effect, provider profile, and completion.

### Decision: Put the real build provider under Lattice

**Choice:** Add a standalone `runtime/kiln-radicle-nix-provider` workspace with a typed profile, explicit source facts, a fixed executable, Bounded Exec limits, deterministic report paths, and its own strict Octet gate.

**Rationale:** Lattice owns workflow execution. Kiln owns command admission, outcome meaning, report identity, and provider policy. Aspen only transports and materializes the provider completion.

### Decision: Keep production publication separate from the nested host lock

**Choice:** Publish the root revision that contains the standalone provider first. Then update the standalone host lock to that immutable Kiln revision and publish the final host commit.

**Rationale:** The nested workspace must not depend on its own unpublished source revision.

## Risks / Trade-offs

- Machine-local Redb, SQLite, and value files do not prove global durability or availability.
- Lattice recovery depends on its exact workflow history and does not prove distributed exactly-once execution.
- Report publication proves retained local bytes, not external report delivery.
- A corrupted durable component must stop the host. Automatic repair is outside this change.
