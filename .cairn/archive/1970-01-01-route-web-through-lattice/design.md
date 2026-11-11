## Context

Web already has a Lattice workflow provider and an authenticated worker lookup/log protocol. The current native Lattice request.v2 deliberately injects only LATTICE_SOURCE_PROVIDER, LATTICE_SOURCE_REPOSITORY and LATTICE_SOURCE_REVISION. It has no arbitrary approval-payload ingress. Preserve that reviewed protocol and native executor; do not modify vendored contracts or invent a forwarding handler.

## Decisions

### Decision: One Web workflow orchestrator

**Choice:** Restore Web's sole LatticeAdapter/UnixWorkflowExchange path. A saved native Lattice workflow invokes a fixed-profile `kiln-hosted lattice-job --profile PATH --identity BLAKE3` shell task. One Lattice node delegates one reviewed hosted pipeline; its backend pipeline jobs are not presented as separate Lattice nodes. A multi-node Web graph is scheduled by Lattice, not reconstructed by the hosted Web adapter.

**Rationale:** Reuses the maintained native route_exact/shell_environment contract, durable native acceptance, cancellation and recovery. The helper is an execution adapter, not another workflow server. Remove the direct composition completely.

**Dependency:** Native execution consumes Lattice source revision `813076d7e42d0ddfb65042bee615bc5f12a2b4d3` from canonical `rad:z2yGPFsQZsUcFNkiyR4Dkj22BuxnC` (declared mirror `https://github.com/OnixResearch/lattice.git`). This cancellation-capable application cohort is distinct from Kiln's vendored contract/transport revision `70496e67c7fd4a8b05914161a8e09de2759bebc8`. Lattice owns the executor; Kiln owns this consumer's exact binary/source receipt and integration proof. Producer-wide test/Octet holds are not cleared by a consumer smoke.

### Decision: Authoritative Web callback rather than worker-supplied approval

**Choice:** Add a Worker/Operator-only read-only `/api/runs/:id/execution-binding` projection returning persisted `plan`, `binding` (run/project/source/untrusted gate), optional native `execution`, `phase`, `outcome`, and existing log bounds. The helper finds one exact active run using the existing project/revision worker lookup, then verifies the native source context, configured node and policy. Hosted policies pin the dedicated Lattice job peer, project/repository, workflow-policy/node/operation identity, exact reviewed hosted pipeline/price/class, and a fixed HTTPS Web service origin/CA/worker credential. Cancellation registration and metadata reservation may precede the callback; before source execution or financial effects, hosted independently fetches and compares the exact Web binding, requiring an active Pending native Lattice execution and approved untrusted gate when present. An unverified reservation is not execution authority.

**Rationale:** Carries existing persisted approval meaning through Lattice without adding an unreviewed generic context transport or trusting arbitrary worker provenance. HTTPS authenticates the configured Web authority; it does not independently prove a human click or an uncompromised Web server.

### Decision: Per-node once-only hosted backend

**Choice:** Use explicit `execution.lattice_run/get/cancel` requests and `lattice_execution` policies. Hash the complete plan, persisted binding, native execution and node into a reserved backend attempt. Retention keys include organization, Web project/run and node. Globally reserve the new attempt/source namespace, retain exact request/config identities and original refusals, and reject changed replay. Read/get/cancel of an existing binding never reacquires or redispatches and need no recursive Web callback. Generic execution.run remains caller-provenance execution.

**Rationale:** Native Lattice may recover the original graph, but cannot turn an uncertain backend task into a second execution. Independent initial Web/source checks preserve approval authority. Current policy/membership checks still apply on recovery.

### Decision: Logs and cancellation follow the actual task

**Choice:** The helper emits and uploads only real bounded hosted stdout/stderr through existing Web worker log ingress. Its signal handler requests exact hosted cancellation, waits for retained cancellation/teardown within the admitted `cleanup_timeout_ms` window, and never reports success after refusal or uncertain cleanup. Because the native runner may destroy a cancelled task without a signal, the Worker execution-binding projection also exposes durable `cancel_requested` intent and hosted observes that intent while its pipeline runs; Web intent, not task liveness, is the cancellation authority that reaches the backend. Native Lattice owns workflow/node status; cancellation intent alone is not completed cancellation. Retain the provider-neutral canonical exact-live reconciliation fix.

## Risks / Trade-offs

- Hosted admission requires the configured Web HTTPS authority to be reachable. Failure refuses a new backend execution; no cached approval fallback or effect retry is introduced.
- The Lattice relay and hosted service hold separate scoped credentials outside guest source. The Web callback must not be performed recursively during Web-held native observation/control; only initial admission uses it.
- Native process cancellation grace must actually permit backend cleanup. Exercise it against the genuine native runtime; do not assume a transport mock proves it.
- Repeated nodes in one Web run have separate backend identities; native graph identities and exact node operation policies prevent cross-node substitution.
- Workstation Linux 6.18.50 remains ineligible for approved-untrusted execution. Actual local trusted execution and correct untrusted refusal do not qualify an eligible host, a distinct physical host, WAN/relay behavior or hostile isolation.

## Verification

Use owned local HTTPS, the real native Lattice saved workflow and exchange process, authenticated hosted Iroh, independently acquired Git and controlled Firecracker guests. Retain native request/graph/workflow/execution evidence and exact binary/source identities. Check missing/stale/moved approval, unpinned relay, modified binding/source/node, replay, actual stdout, live cancellation, restart and owned teardown. Preserve prior direct-route receipts as superseded, never as proof of this topology. No remote publication, production mutation or lifecycle archive authority is granted.
