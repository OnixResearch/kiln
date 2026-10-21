# Tasks

## Phase 1: Scope and admission

- [x] [serial] Inspect same-host node/source/status dependencies and probe the second host's LAN and tailnet transport; record the reachable boundary without mutation. r[kiln.radicle_ci.distinct_host.authority] r[kiln.radicle_ci.distinct_host.transport]
- [x] [serial] Record the actual SSH destination, remote tool/store availability, remote root rules and native interfaces; admit proposal, design and tasks before implementation. r[kiln.radicle_ci.distinct_host.authority]

## Phase 2: Distinct-host implementation

- [x] [serial] Add typed remote host/root/address admission with positive and negative fixtures while preserving the existing same-host default. r[kiln.radicle_ci.distinct_host.authority]
- [x] [serial] Add the bounded remote shell: fresh mode-0700 remote root, exact file transfer, process start/stop, readiness and remote retirement. r[kiln.radicle_ci.distinct_host.authority] r[kiln.radicle_ci.distinct_host.evidence]
- [x] [serial] Provision the publisher node and source worktree remotely; connect the unchanged local CI node directly to the advertised tailnet address; keep broker, controller, worker and observer unchanged. r[kiln.radicle_ci.distinct_host.transport]
- [x] [serial] Route publisher `rad`/`jj` operations remotely, mirror each exact source commit, and retain remote commands, logs, identities and cleanup. r[kiln.radicle_ci.distinct_host.transport] r[kiln.radicle_ci.distinct_host.evidence]

## Phase 3: Distinct-host proof

- [x] [serial] Exercise genuine remote success with automatic broker ingress, local controller host kill/reconcile, exactly one worker invocation and independent local status readback. r[kiln.radicle_ci.distinct_host.transport] r[kiln.radicle_ci.distinct_host.status] r[kiln.radicle_ci.distinct_host.recovery]
- [x] [serial] Exercise real duplicate delivery, remote publisher restart/reconnect, source-controlled build failure and unavailable-provider uncertainty; retain original-operation and root-discharge evidence. r[kiln.radicle_ci.distinct_host.recovery] r[kiln.radicle_ci.distinct_host.evidence]
- [x] [serial] Retain exact remote topology, addresses, process/log facts, ingress, journals and status; state the same-host status boundary and private-overlay non-claims. r[kiln.radicle_ci.distinct_host.evidence] r[kiln.radicle_ci.distinct_host.claims]

## Phase 4: Delivery

- [x] [serial] Run focused tests and every required repository gate; synchronize and archive only after the distinct-host prerequisites and acceptance criteria actually pass. r[kiln.radicle_ci.distinct_host.evidence]
