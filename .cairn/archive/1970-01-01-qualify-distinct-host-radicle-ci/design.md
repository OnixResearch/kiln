# Design: distinct-host publisher with local CI authority

## Context

Reuse `kiln-radicle-ci`, the reviewed project worker, the existing Aspen/Lattice
runtime and the same-host live runner. The previous qualification starts at a
native peer update but places all three Radicle nodes on one host. The new
boundary keeps CI authority local and moves only the source publisher and its
worktree to a second owned host. Delivery then crosses a non-loopback TCP
connection. Status readback remains the already-qualified local independent
observer boundary and is declared as such.

## Decisions

### Decision: Move only the publisher

**Choice:** The publisher node, its keys and its source worktree live on the
remote host. The CI node, broker, controller, project worker, Aspen host,
Lattice and observer remain on the local host. The local CI node connects to the
publisher's advertised address and seeds the exact RID.

**Rationale:** The installed broker subscribes through the local Radicle node
control socket; moving the CI node would also move the broker, controller and
worker, expanding the change far beyond the missing source-delivery boundary.
Moving the publisher isolates the question: can a genuine native update from
another host enter the unchanged broker and produce the same canonical and
status results? The status readback non-claim remains explicit.

### Decision: Private tailnet transport, not a tunnel

**Choice:** The publisher node listens on the remote host's tailnet address and
advertises it. The local CI node opens the Radicle TCP connection directly over
the tailnet. SSH is used only to provision files, start/stop the remote node and
run publisher commands. No forwarded data port is used.

**Rationale:** Direct host-to-host TCP is the behavior under test. A LAN probe
showed inbound LAN ports are firewalled in both directions, while the private
tailnet accepts connections both ways. The evidence must name the tailnet
boundary and must not promote it to WAN or hostile-network behavior. A tunnel
would hide the transport and is rejected.

### Decision: Remote admission is explicit and fresh

**Choice:** Extend the typed live profile with a `remote` block: SSH destination,
canonical fresh `/var/tmp` root, tailnet address, node port and readiness
deadline. The local runner validates the local and remote roots before effects,
creates the remote root atomically with mode 0700, and refuses pre-existing
state. The publisher home is created locally, copied over SSH, and started
remotely; the remote keys and home are retired after the run. No operator
credential or existing Radicle home is read.

**Rationale:** The same-host admission contract already refuses foreign roots and
identities. Extending it with an explicit remote fact set keeps effects bounded
and reviewable, and the positive/negative fixtures cover bad hosts, roots,
addresses and missing tools.

### Decision: Preserve the existing CI and status contracts

The broker configuration, trigger filters, controller profile, workflow,
worker and status profile are unchanged. The only routing differences are the
publisher node's listen/advertise address, the CI node's `node connect` target,
and executing publisher `rad`/`jj` commands through the remote shell. The local
source worktree remains the content authority for the expected Nix output; after
each local commit it is mirrored to the remote publisher worktree, where the
exact same commit is pushed to the remote node.

The status checkout remains local and uses the existing Radicle URLs for the CI
and publisher namespaces. The independent observer remains local. The evidence
must state that status dissemination stayed same-host while source ingress
became distinct-host.

### Decision: Exercise real transitions, not only a happy path

Run the same three cases as the same-host canary: success with local controller
host kill/reconcile while the worker continues, source-controlled build failure,
and unavailable provider with bounded nonterminal uncertainty. Perform a real
duplicate fetch. Additionally restart the remote publisher node after the first
successful delivery and prove the local CI node reconnects and preserves the
original operation without redispatch. Retain remote process logs and the exact
remote command bytes.

## Risks / Trade-offs

- Tailnet availability and remote firewall behavior are operator-owned. Missing
  connectivity is a bounded prerequisite failure, not a reason to fall back to
  a loopback or tunneled claim.
- The remote Nix store differs from the local store. Source locking and the
  expected-output evaluation stay local; only commits and pushes run remotely.
  This avoids claiming remote Nix equivalence.
- Copying the publisher home over SSH carries test private keys to the remote
  host. They are dedicated non-production canary keys, mode-restricted, and are
  deleted from both hosts at teardown.
- The remote publisher process is owned through SSH; if the SSH control
  connection or remote host is lost, the run must fail as an uncertain
  prerequisite and still attempt remote retirement.
- Distinct-host source delivery does not prove WAN, hostile-network, remote
  worker confinement, production deployment or release authority.

## Evidence and lifecycle

Retain the exact remote host, tailnet address, SSH command shape, remote root,
node IDs, listen/advertise configuration, publisher process identity and logs,
source revisions, broker ingress and replies, canonical journals, local status
readback, root discharge, duplicate/restart observations, rejected attempts and
remote retirement. Exclude private keys and seed values. Gate proposal, design
and tasks before implementation; run focused tests, all repository gates, then
synchronize and archive only with actual evidence.

Kiln maintainers own the repeatable qualification. The operator owns the remote
host, SSH access, tailnet, firewall and cleanup. This change does not deploy a
persistent service or activate built outputs.
