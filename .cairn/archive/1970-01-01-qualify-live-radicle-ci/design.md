# Design: genuine native peer and broker canary

## Context

Reuse `kiln-radicle-ci`, the reviewed project worker and the maintained Aspen/Lattice/Nix cohort. The previous runner fabricates neither provider results nor CI outcomes, but it starts at a manually requested producer message and fixture status credentials. The new boundary starts at a real peer update and ends at a separately fetched signed status object.

## Decisions

### Decision: Explicit fresh-state native topology

**Choice:** Provision separately owned publisher and CI node profiles plus independent observer storage, all under one fresh private canary root. Configure only the declared canary peers, repository and branch; disable ambient/default seed connectivity. Use actual Radicle node networking and the installed broker's native event subscription. Record addresses, node IDs, host placement and exact upstream configuration. Different node processes on one host establish native transport, not distinct-host or WAN behavior.

**Rationale:** Real native transport, signed refs and upstream broker filtering are the missing integration boundary. Dedicated identities and explicit peer topology avoid granting the existing operator node, production services or public repositories any mutation authority. Source and status identities remain distinct; production-profile admission and real SecretSpec resolution are mandatory.

### Decision: Preserve the existing CI and status contracts

The broker invokes the controller's normal `--profile` protocol. Its input and replies are retained unchanged; any wrapper may capture bytes and process lifecycle but must not construct events, alter fields, fabricate outcomes or supply a fixture signer. Configure one unchanged reviewed workflow/profile across revisions. Canonical reservation and actual worker results remain completion authority. Broker-native job records, process exits and remote statuses are evidence projections only.

The canary shell explicitly owns status-ref signing and peer synchronization after the controller's dedicated exact intent/write observation. It must not add implicit networking to the pure CI core or local status shell. The observer uses the same bounded reducer and single-writer/revision admission over independently fetched storage, and verifies exact object/head equality. Source signing does not grant status-signing authority.

### Decision: Exercise live transitions, not only a happy path

Deliver an actual new source revision with its consumer output absent; record broker-native node event and controller input before checking the canonical run and externally fetched exact status. Deliver a real source-controlled Nix build failure. Interrupt controller/host while the real worker independently runs, restart/reconcile the original operation, and prove one invocation. Exercise duplicate native delivery/recovery and missing-provider observation without manufacturing a final reply or status. Retain rejected/failed attempts and distinguish setup failures from qualified cases.

Pure admission and classification remain deterministic and I/O-free. The Python/native shell owns files, process limits, clocks, networks, credentials, source changes and transport. New permanent tests cover uncertain consumer-visible boundaries only; actual services are exercised by the repeatable canary, not unit tests.

## Evidence and lifecycle

Record the parent source revision and changed-file identities, immutable executable/profile/workflow identities, native node/broker versions, declared topology, original command/input/output bytes, source/signed-ref identities, canonical journal and original operation, actual Nix output/diagnostic, local and independently fetched status, explicit root discharge and private credential/process retirement. Exclude all private key/seed values. Run proposal/design/tasks gates before implementation and required focused/repository gates after integration. Sync/archive only requirements with their actual evidence; absent prerequisites cannot be replaced with fixture results or declared PASS.

Kiln maintainers own this repeatable qualification and its current canary consumer. OnixOS remains the owner of later persistent deployment. This change neither deploys persistent production services nor activates consumer outputs.


## Risks / Trade-offs

- Native broker filtering, native runtime revisions and the separately pinned COB storage cohort can differ; inspect actual upstream contracts and fail on incompatible identities rather than changing pins to manufacture a pass.
- The broker may create its own CI-job projection. Keep its object and signer distinct from Kiln's exact-revision canonical projection and never use it as CI completion authority.
- Source/worker state currently shares a user. Only trusted reviewed canary code is allowed; no hostile-worker confinement, hermeticity or production security claim is made.
- Signer resolution clears the environment. Use a declared real SecretSpec provider compatible with that contract, never a fixture resolver or an embedded secret default.
- Same-host native peer processes do not establish WAN delivery or independent physical hosts. The retained topology and final claim must state the exercised network boundary.
