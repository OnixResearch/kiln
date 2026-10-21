## ADDED Requirements

### Requirement: Dedicated distinct-host authority and fresh remote state

r[kiln.radicle_ci.distinct_host.authority] The distinct-host qualification MUST use one explicitly declared SSH destination, one canonical fresh `/var/tmp` remote root, dedicated source/status identities, the exact pinned executable cohort and finite readiness, peer and process deadlines. It MUST refuse pre-existing or foreign remote state, a non-canonical or non-`/var/tmp` root, an undeclared address or a missing pinned tool before any node, source or status effect. It MUST NOT use or mutate the operator's existing Radicle home, production repository, signer, service, governance, visibility, mirror or main refs.

#### Scenario: Owned distinct-host canary starts

- GIVEN a fresh local root, a fresh declared remote root, the exact SSH destination and the declared tailnet address
- WHEN distinct-host admission and remote provisioning run
- THEN every remote effect MUST stay inside the declared root and owned process set.

#### Scenario: Remote state or authority is not fresh

- GIVEN a pre-existing remote root, an undeclared host or address, a foreign RID/branch or a missing pinned executable
- WHEN admission runs
- THEN it MUST reject before remote source, signing or network effects.

### Requirement: Genuine distinct-host native delivery

r[kiln.radicle_ci.distinct_host.transport] The publisher Radicle node and its source worktree MUST run on the declared remote host. That node MUST listen and advertise on the declared non-loopback private address, and the unchanged local CI node MUST open the Radicle TCP connection directly to it. SSH MAY provision files and start or stop owned processes but MUST NOT carry the source-data path or forward the Radicle port. The installed broker MUST invoke the unchanged controller/worker workflow for the exact remote revision without a manufactured request or manual `cibtool trigger`. The remote node event, original broker input and replies, exact signed source and canonical reservation MUST be retained.

#### Scenario: A remote-node revision arrives

- GIVEN the remote publisher and local subscribed broker
- WHEN a genuine native peer update crosses the declared address
- THEN exactly one canonical source-bound operation MUST be reserved and the local worker MUST receive the actual source bytes.

#### Scenario: Delivery is loopback, tunneled or manufactured

- GIVEN loopback-only delivery, an SSH-forwarded data port, a manual producer request or a fixture provider result
- WHEN the distinct-host criterion is evaluated
- THEN it MUST NOT be reported as genuine distinct-host native ingress.

### Requirement: Declared status boundary and independent readback

r[kiln.radicle_ci.distinct_host.status] The canary MUST retain the existing local CI/observer status boundary and MUST state that status dissemination remained same-host. After canonical terminal admission it MUST use the normal status service, dedicated canary signer and pinned SecretSpec executable, explicitly sign/synchronize only the declared canary refs, and independently fetch and reduce the same status in observer storage. Repository, source revision, run, outcome, object ID and head MUST agree. Distinct-host source delivery MUST NOT be promoted to distinct-host status dissemination.

#### Scenario: Exact local status is independently visible

- GIVEN an admitted terminal canonical result from a distinct-host source
- WHEN canary-only signing/synchronization and observer fetch complete
- THEN bounded read reconciliation MUST return the same revision/run/outcome/object/head and the evidence MUST state the same-host status boundary.

#### Scenario: Status evidence is absent or foreign

- GIVEN a missing independent object, a foreign signer or a divergent head
- WHEN status qualification runs
- THEN the criterion MUST fail and no fixture credential or broker projection MAY replace it.

### Requirement: Remote restart, duplicate and host-loss recovery

r[kiln.radicle_ci.distinct_host.recovery] During actual distinct-host delivery the canary MUST restart the remote publisher node and MUST exercise duplicate delivery plus local controller/host loss while the real worker independently executes. After remote restart the local CI node MUST reconnect, reconcile the original operation and retain exactly one worker invocation. Duplicate native fetch MUST preserve the original operation and invocation count. Canonical recovery MUST NOT redispatch after uncertain acceptance.

#### Scenario: The remote publisher restarts

- GIVEN one real remote-delivered source event and its canonical operation
- WHEN the remote publisher node is stopped and restarted and the local CI node reconnects
- THEN the original source/run/attempt and operation MUST remain and no second worker invocation MUST occur.

#### Scenario: Duplicate or lost host cannot establish acceptance

- GIVEN a replayed event, an interrupted controller or a lost remote process observation
- WHEN recovery runs
- THEN it MUST retain uncertainty and the original operation rather than invent a new operation or success.

### Requirement: Bounded distinct-host evidence and claims

r[kiln.radicle_ci.distinct_host.evidence] Kiln maintainers MUST retain the exact SSH destination, remote root, tailnet address and firewall observations, remote process identities and logs, node IDs, source revisions, broker ingress and replies, canonical journals, independently fetched local status, root discharge, rejected attempts, remote retirement and the exact repeat command. They MUST run required affected and repository gates and synchronize/archive only after the actual prerequisites pass. Two owned hosts on a private overlay MUST NOT be promoted to WAN, hostile-network, hostile-worker confinement, production deployment, activation or release authority.

#### Scenario: Complete distinct-host canary repeats

- GIVEN the recorded cohort, remote prerequisites and fresh local/remote state
- WHEN all declared cases and gates execute
- THEN the retained evidence MUST reproduce remote peer-to-local-broker-to-CI ingress with explicit network and status-boundary non-claims.

#### Scenario: Remote prerequisite or authority is unavailable

- GIVEN an unreachable host, a blocked address, a missing pinned tool or absent mutation authority
- WHEN the qualification is evaluated
- THEN the missing criterion MUST remain explicit and no loopback, tunneled, fixture or historical result MAY replace it.

### Requirement: Bounded distinct-host claims

r[kiln.radicle_ci.distinct_host.claims] The retained evidence MUST identify the two owned hosts and the private overlay as the exercised network boundary. It MUST state that status dissemination remained same-host. It MUST NOT promote the run to WAN, hostile-network, hostile-worker confinement, production deployment, activation or release authority.

#### Scenario: Evidence is interpreted

- GIVEN the retained distinct-host observations
- WHEN claims are projected
- THEN they MUST name the private overlay, the same-host status boundary and every excluded stronger claim.

#### Scenario: A stronger claim is requested

- GIVEN a request to call the run WAN, hostile-network, production or release-qualified
- WHEN the evidence is evaluated
- THEN the request MUST be rejected and the actual boundary retained.
