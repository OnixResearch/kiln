# Qualify distinct-host Radicle CI delivery

## Why

The same-host canary proves native peer delivery, broker dispatch, canonical
results and independent status readback, but it explicitly excludes
distinct-host delivery. The missing uncertainty is whether the installed broker
and unchanged controller/worker path still produce the qualified boundary when
the source node and its signed update live on another machine and delivery
crosses a real non-loopback TCP connection. The current consumer is the same
trusted canary; no provider abstraction or general remote-execution feature is
needed.

## What Changes

- Add a typed distinct-host canary profile: explicit SSH orchestration
  destination, canonical fresh remote root, tailnet listen/advertise address,
  finite readiness/peer deadlines and an explicit same-host status boundary.
  r[kiln.radicle_ci.distinct_host.authority]
- Run only the publisher node and its source worktree on the remote host. Keep
  the CI node, broker, controller, project worker and observer local. The CI
  node connects to the publisher's advertised tailnet address; SSH is
  orchestration only and never the source-data path.
  r[kiln.radicle_ci.distinct_host.transport]
- Deliver genuine native peer updates across hosts, retain the original broker
  ingress, canonical journals, real success/failure outcomes, duplicate and
  restart recovery, and independent local status readback. Retain remote node
  logs, addresses, process identities and retirement.
  r[kiln.radicle_ci.distinct_host.evidence]
- State that two owned hosts on a private tailnet are not WAN, hostile-network,
  hostile-worker confinement, production deployment or release authority.
  r[kiln.radicle_ci.distinct_host.claims]

## Impact

Current consumer and immediate outcome: the trusted canary obtains automatic
source-event CI from a publisher on a distinct host, with the same canonical and
status guarantees. Durable capability: a repeatable distinct-host placement and
evidence rail for later qualification of remote sources.

Maintenance owner: Kiln maintainers own the runner, admission and evidence.
The operator owns the SSH destination, tailnet membership, host firewall,
remote root, test identities and cleanup.

Files: the existing same-host runner plus a distinct-host exercise/profile and
remote-host shell; typed Nickel admission and positive/negative fixtures;
focused regressions only for new admission or routing behavior; operator
documentation and Cairn evidence. The same-host rail remains the default and
unchanged.

Verification: actual remote publisher native delivery into the local CI node,
success and genuine source failure, duplicate suppression, local controller
host-kill recovery, unavailable-provider uncertainty, exact status readback and
remote retirement. Publication and process exit codes alone are not evidence.

## Authority and exclusions

The operator's `begin` request authorizes this dedicated canary, including fresh
test identities, a fresh remote `/var/tmp` root, SSH orchestration to the named
host, source pushes and status signing confined to its exact RID. It does not
authorize changes to the operator's existing Radicle homes, production
repositories, signers, services, governance, visibility, mirrors or `main`, nor
activation of built outputs or release approval. The remote host runs only
reviewed canary commands and owned processes. No untrusted repository is
admitted. A private tailnet run must be identified as such and cannot be called
WAN, hostile-network or production qualification.
