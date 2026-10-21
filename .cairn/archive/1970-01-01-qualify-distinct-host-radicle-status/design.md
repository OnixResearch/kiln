# Design: remote observer with local status authority

## Context

Reuse the qualified distinct-host publisher runner. That exercise moves only the
publisher and proves direct tailnet source delivery. This change adds the second
host to the status path: the observer node and its storage move remote, while
the controller, status intent, signing and status-ref synchronization stay with
the local CI authority. The remote read uses the same pinned coordinator
`status-reconcile` command already qualified for read-only storage reduction.

## Decisions

### Decision: Keep status creation and signing local

**Choice:** The controller persists the exact status intent, writes the local
status object and synchronizes the declared checkpoint ref exactly as before.
Only the observer node, its storage and the readback move to the second host.

**Rationale:** The security and authority boundary is the signer and the
single-writer status service. Moving them would require a different authority
model. Moving only the observer tests dissemination and independent reduction
without changing who is allowed to create status.

### Decision: The local CI node listens on the tailnet for the observer

**Choice:** Restart the local CI node before the observer move with
`listen = 0.0.0.0:<ci_port>` and `externalAddresses = [<local_tailnet>:<ci_port>]`.
The remote observer opens the Radicle TCP connection directly to that address.
The local broker continues to use the local control socket.

**Rationale:** The remote observer must initiate a connection to the CI node.
The private tailnet accepts inbound TCP in both directions (probed), while LAN
inbound is firewalled. SSH remains orchestration only; no status port is
forwarded. The broker and controller are unaware of the address change.

### Decision: Reuse the remote root and publisher shell

**Choice:** The observer home occupies `<root>/rad-observer` on the same remote
host and root as the publisher. The runner stops the local observer, copies its
home, starts the remote node on loopback (it only needs outbound connectivity),
connects it to the local CI tailnet address, and seeds the exact RID. The
publisher and observer are retired together by removing the declared root.

**Rationale:** One bounded remote root keeps cleanup atomic. The observer does
not need an advertised inbound address because it initiates the connection.

### Decision: Remote reduction through the pinned command

**Choice:** Copy the exact `status-profile.json` and the persisted
`<case>-status-plan.json` to the remote root and run
`kiln-radicle-ci status-reconcile PROFILE REMOTE_STORAGE PLAN` there with the
pinned store binary. Require the output to equal the locally written status
object exactly, and require the pre-sign read to return no object.

**Rationale:** Reading the remote storage only by copying it back would not
prove that the remote host can independently reduce the fetched object with the
qualified read path. The coordinator command is read-only and does not resolve
credentials, sign or network.

### Decision: Preserve the existing cases and claims

Run the same success, source-controlled failure and unavailable-provider cases.
The unavailable case must retain no status and no remote object. Status
dissemination and readback become distinct-host; signing and object creation
remain local. The network boundary remains a private tailnet between two owned
hosts, not WAN or a hostile network.

## Risks / Trade-offs

- The local CI node must accept tailnet inbound traffic; a firewall change is a
  prerequisite, not a reason to fall back to an SSH-forwarded data port.
- The remote observer storage is on a different filesystem. Reconcile must use
  the explicit remote storage root and must not read the local observer copy.
- Restarting the CI node for the tailnet listen resets its peer table; the
  runner must reconnect the remote publisher before the cases.
- The remote coordinator binary and status profile must be present and
  compatible on the second host; a missing pinned binary is an explicit
  prerequisite failure.
- The remote observer is a trusted canary identity. This does not prove
  hostile-node isolation or production status dissemination.

## Evidence and lifecycle

Retain the remote observer host/home/storage identity, CI tailnet listener and
external address, observer outbound connection lines, remote publisher and
observer logs, remote commands, pre-sign absence and exact readback for each
terminal case, local status intent/observation, canonical journals, retirement
and gates. Exclude private keys and seed values. Gate proposal, design and tasks
before implementation; run focused and repository gates; synchronize and
archive only after the distinct-host status prerequisites actually pass.

Kiln maintainers own the repeatable qualification. The operator owns the remote
host, SSH access, tailnet, firewall and cleanup. This change does not deploy a
persistent service or activate built outputs.
