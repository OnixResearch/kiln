## Why

The Firecracker backend gives each job a guest with no network interface. That is
deliberate, and it carries the network part of the isolation claim, but the
operator guide only describes current behaviour. Nothing states that guest
networking is a non-goal or what design would be acceptable if a job ever needed
egress, so the easiest future change would be to add a TAP device, which would
silently invalidate the escape-suite result and the isolation claim. Issue #23
asks for that decision to be written down.

## What Changes

- Add a "Guest networking (non-goal)" subsection to the Firecracker backend in
  `docs/hosted-ci.md`, linked from the isolation-claims paragraph.
- State that the absent guest interface (no TAP, no virtio-net, no in-guest
  tunnel) is part of the isolation claim, not a missing feature.
- State the preferred design if egress is ever needed: host-mediated over the
  existing vsock channel, with a reviewed allowlist, size and time bounds, and
  every fetch recorded in the job receipt; the guest still gets no IP path.
- List the rejected alternatives and why: a guest NIC on a host bridge or NAT, and
  an in-guest or host-terminated L3 tunnel including userspace WireGuard such as
  sanstun.
- Require any change that adds guest networking to update the isolation claim and
  the escape suite in the same change, naming the suite's target list.

## Impact

- **Files**: `docs/hosted-ci.md` only. No code, configuration, receipts or
  isolation claims change.
- **Testing**: source review plus the Cairn document checks (`validate`,
  traceability coverage, release-readiness). No test repeats the prose.
