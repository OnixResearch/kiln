# Live Radicle Ci Distinct Status Specification

## Purpose

Defines the `live-radicle-ci-distinct-status` capability.

## Requirements

### Requirement: Explicit remote observer authority

r[kiln.radicle_ci.distinct_status.authority] The qualification MUST use the declared SSH destination, one canonical fresh `/var/tmp` root shared by the publisher and observer on the remote host, a declared local CI tailnet address, dedicated observer identity and the exact pinned coordinator/profile cohort. It MUST refuse a non-fresh root, a non-tailnet local or remote address, a missing pinned remote coordinator, an undeclared host or missing mutation authority before any observer, status or network effect. It MUST NOT mutate the operator's existing Radicle homes, production repositories, signers, services, governance, visibility, mirrors or main refs.

#### Scenario: Remote observer starts

- GIVEN a fresh local root, a fresh declared remote root, the declared host and both tailnet addresses
- WHEN admission and remote observer provisioning run
- THEN every effect MUST stay inside the declared roots and owned process set.

#### Scenario: Admission is not fresh or complete

- GIVEN pre-existing remote state, an undeclared address, a missing pinned coordinator or a foreign identity
- WHEN admission runs
- THEN it MUST reject before observer, status or network effects.

### Requirement: Direct distinct-host status transport

r[kiln.radicle_ci.distinct_status.transport] The local CI node MUST listen on the declared local tailnet address while retaining its local control socket, and the remote observer node MUST open the Radicle TCP connection directly to it. Status creation, signing and checkpoint-ref synchronization MUST remain local and unchanged. SSH MAY provision files and control owned processes but MUST NOT carry the status data path or forward the Radicle port.

#### Scenario: Remote observer connects

- GIVEN the local CI tailnet listener and the remote observer node
- WHEN the observer connects and seeds the exact RID
- THEN the CI node log MUST show the outbound remote peer and the observer log MUST show the inbound local CI peer.

#### Scenario: Transport is loopback, tunneled or undeclared

- GIVEN an SSH-forwarded status port, a loopback-only observer or an undeclared address
- WHEN the distinct-host status criterion is evaluated
- THEN it MUST NOT be reported as genuine distinct-host status transport.

### Requirement: Exact remote status readback

r[kiln.radicle_ci.distinct_status.readback] After the local status service writes the exact status object and the canary synchronizes only the declared checkpoint ref, the remote observer MUST fetch that ref into its own storage and reduce it there with the pinned `kiln-radicle-ci status-reconcile` command. The reduced repository, revision, run, outcome, object ID and head MUST equal the local written observation exactly. The pre-sign read against the same remote storage MUST return no object. The unavailable-provider case MUST retain no status and no remote object.

#### Scenario: Exact status is remotely reducible

- GIVEN a written and signed terminal status for an admitted source revision
- WHEN the remote observer fetch and remote reduction complete
- THEN every identity field MUST match the local observation and the pre-sign read MUST have been absent.

#### Scenario: Remote reduction is absent or divergent

- GIVEN a missing remote coordinator, a missing fetched object or a divergent head/run
- WHEN the status criterion is evaluated
- THEN it MUST fail and no local observer copy, fixture credential or broker projection MAY substitute.

### Requirement: Bounded distinct-host status evidence

r[kiln.radicle_ci.distinct_status.evidence] Kiln maintainers MUST retain the remote observer host/home/storage identity, local CI listener and external address, observer connection lines, remote observer log, remote commands, pre-sign absence and exact readback for terminal cases, the unavailable no-status result, canonical journals, local status intent/observation, retirement and the exact repeat command. They MUST run required gates and synchronize/archive only after actual prerequisites pass.

#### Scenario: Complete remote status canary repeats

- GIVEN the recorded cohort, remote prerequisites and fresh state
- WHEN all declared cases and gates execute
- THEN the retained evidence MUST reproduce remote status fetch and reduction with explicit local-authority and network non-claims.

### Requirement: Bounded distinct-host status claims

r[kiln.radicle_ci.distinct_status.claims] The evidence MUST state that status object creation and signing remained local while dissemination and readback became distinct-host. It MUST NOT promote the private tailnet to WAN, hostile-network, hostile-node isolation, production deployment, activation or release authority.

#### Scenario: A stronger claim is requested

- GIVEN a request to call the run WAN, hostile-node, production or release-qualified
- WHEN the evidence is evaluated
- THEN the request MUST be rejected and the actual local-authority/private-overlay boundary retained.
