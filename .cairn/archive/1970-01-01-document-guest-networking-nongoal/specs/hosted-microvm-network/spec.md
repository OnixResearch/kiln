## ADDED Requirements

### Requirement: Guest networking is a documented non-goal

r[kiln.hosted.microvm.network.nongoal]
`docs/hosted-ci.md` MUST document, under the Firecracker backend and linked from
the isolation-claims paragraph, that a job gets no guest network interface (no
TAP, no virtio-net, no in-guest tunnel) as a deliberate part of the isolation
claim. It MUST state that host-mediated egress over the existing vsock channel,
with a reviewed allowlist, size and time bounds, and receipt-recorded fetches is
the preferred design if egress is ever needed, and that the guest still never
receives an IP path. It MUST reject a guest NIC on a host bridge or NAT and an
in-guest or host-terminated L3 tunnel, including userspace WireGuard, because they
give the guest general IP reachability. It MUST require any change that adds guest
networking to update the isolation claim and the escape suite in the same change.
No code, configuration, receipt or isolation claim changes.

#### Scenario: The escape suite is named as the coupled check

r[kiln.hosted.microvm.network.nongoal.scenario.escape]
- GIVEN the new subsection
- WHEN a reader looks for the check that must change if guest networking is added
- THEN the escape suite's TCP targets `1.1.1.1:443`, `10.0.2.2:80` and
  `169.254.169.254:80` and its vsock probes MUST be named as that check
