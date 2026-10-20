# Maintained-cohort OnixOS pilot design

## Context

The standalone Aspen host deliberately resolves its Kiln dependencies from immutable revision `69afc2697f4330005797256135ae81b2722a125c`. Its operator ingress is versioned, while its reviewed Lattice effect planner preserves Radicle repository/revision facts. The existing GitHub/Iroh rail is independent; it is not an existing GitHub-to-Aspen bridge. An Aspen terminal acknowledgement alone is not canonical CI state or proof of the worker's build result.

The root workspace's separately completed Onix implementation is preserved as its own predecessor revision. Its exact evaluator/plugin/linked-library cohort stays fixed. Its source gate must cease using a single test fixture as the only possible consumer.

## Decisions

### Decision: Explicit source binding, not a second hard-coded consumer

**Choice:** Add a typed Onix source binding to the explicitly selected Onix profile. Bind a direct immutable Nix store source root, its measured `flake.nix` BLAKE3, one root selector and `x86_64-linux`. Check the request against that binding and measure that bound flake before GC-root creation or execution. Require callers to supply the binding explicitly; stock profiles carry no Onix source binding and retain their serialized shape.

**Rationale:** A fixed evaluator cohort and an explicitly admitted consumer are separate facts. Adding another source constant or silently accepting arbitrary request paths would preserve the symptom or weaken admission. The fixture remains a valid explicit conformance consumer, not a default runtime source or build qualification.

### Decision: Authenticated operator initiation without forge mutation

**Choice:** Use fresh operator Ed25519 credentials in private scratch and an explicitly allowlisted public-key profile. Sign the exact bounded request under a pilot-specific domain; verify signature, source/cohort bindings and freshness before any host/provider effect. The existing operator Aspen ingress carries genuine published Radicle facts; the immutable OnixOS source and job selector are independently declared workflow inputs. Persist logical request identity before dispatch so redelivery cannot create a second operation.

**Rationale:** This exercises authenticated real operator initiation without inventing GitHub deliveries, modifying production webhooks, or adding control methods to the read-only observation service. It is not evidence of autonomous forge delivery or of a GitHub-to-Aspen adapter.

### Decision: Real existing OnixOS application as the consumer

**Choice:** Freeze the reviewed OnixOS source and its original locked inputs. Select its existing home-activation application generated from `examples/home-activation/inventory.ncl` and the real `home-tools` service through the Nickel/WASM shim. A small explicit jobset projection may select that actual application without replacing its implementation. Record any explicitly selected producer inputs; incompatible evaluator/plugin inputs fail closed rather than fall back. Build the actual application with the exact existing Nix build adapter. Do not execute activation against a real home.

**Rationale:** The resulting executable application is a bounded real consumer artifact; the discovery-only fixture and an echo-only derivation are not substitutes. Provisioning/acquisition is a separate recorded boundary; the build adapter retains its local-input, no-substitution policy.

### Decision: Durable canonical journal and bounded observation

**Choice:** The pilot coordinator feeds real admitted host/provider observations into existing pure `decide`/`evolve` transitions and persists their versioned event journal. Start intent precedes external dispatch. A terminal event requires the exact native host acknowledgement and the actual worker/build result for that operation. Replay reconstructs the same state; an interrupted intent remains uncertain until existing reconciliation supplies an observation. HTTP observation consumes only the replayed state and admitted event/log windows.

**Rationale:** Neither synthetic `State` construction nor relabeling an Aspen operation file as an event journal establishes canonical CI results. The core remains free of files, clocks, processes and provider implementations.

### Decision: Independently owned provider execution across host restart

**Choice:** Run the real maintained host/extension, real Lattice and worker as isolated owned processes with private sockets, state and finite deadlines. Interrupt/restart only the pilot host/controller while the separately owned workflow continues. Redeliver the same logical request and reconcile the original operation. Retain provider invocation identities/counts and terminal acknowledgements. Retain output GC roots until the result/journal handoff is durable, then exercise explicit idempotent release.

**Rationale:** A replayed result must not be proved by starting another build. Unknown outcomes remain unknown; a report or process exit alone is never promoted to successful CI.

### Decision: Fresh UCAN observation credentials and truthful receipts

**Choice:** Use fresh loopback TLS and UCAN issuer/holder credentials, an exact anchored auth profile and the existing HTTPS observation service. Query actual replayed run/attempt state; verify view identity and canonical outcomes. Exercise wrong-holder/target/replay denials. Keep private keys and tokens outside retained source/evidence. Retain actual commands, outputs, source identities, executable/profile hashes, logs, journal identity, roots and failures.

**Rationale:** Existing unit conformance or a copied fixture response does not qualify this live cohort. Admission/linkage is not issuer intent, provider truth, confinement, general availability or production readiness.

## Risks / Trade-offs

- Original OnixOS inputs may require operator read credentials or separate provisioning. Capture the exact inputs and failures; do not edit upstream locks or hide missing inputs with ambient lookup.
- Maintained runtime and current coordinator use separately pinned source cohorts. Their versioned wire contract must be measured and exercised; historical derivation equivalence is not a new runtime pass.
- Rooted builds intentionally disable substitution and remote builders. Prepare the admitted local closure separately; missing inputs remain a real negative outcome.
- A local operator-triggered integration pilot does not establish autonomous authenticated forge delivery, production cutover, remote status publication or whole-system/activation safety.
- A finite recovery exercise proves its recorded operation only, not distributed exactly-once execution or storage-device durability.
