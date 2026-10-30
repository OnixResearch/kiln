## Context

- `crates/kiln-microvm/src/plan.rs` writes a Firecracker configuration with no
  `network-interfaces` member, one read-only source drive, one vsock device and an
  entropy device.
- `docs/hosted-ci.md` says "There is no network or jailer field", that the guest
  has no network device, and that the escape suite denied IP and vsock egress. It
  does not state the decision or the acceptable design if egress were needed.
- `crates/kiln-hosted/tests/microvm.rs` runs the retained escape suite inside the
  guest: TCP probes to `1.1.1.1:443`, `10.0.2.2:80` and `169.254.169.254:80`, plus
  vsock probes on ports 10240, 1024 and 22, each of which must report `denied`.
- Kiln's own transport (Iroh) runs on the host, not in the guest; the debug hold
  relays commands over the guest's existing vsock session and adds no network
  path.

## Goals and Success Contract

A reader of the Firecracker section learns that the absent interface is a
deliberate non-goal that carries part of the isolation claim, what host-mediated
design would replace it if egress were ever required, which alternatives are
rejected, and exactly which check must change with any future networking change.
The guide's behaviour statements, the escape suite and receipts stay as they are.

## Decisions

### Decision: document the invariant, not a feature

**Choice:** one subsection under the Firecracker backend, linked from the
isolation-claims paragraph. No code, configuration or receipt changes.

**Rationale:** the issue is speculative and docs-only; writing the decision down
is what prevents an accidental TAP device from quietly weakening the claim.

### Decision: host-mediated egress over the existing vsock channel

**Choice:** if egress is ever needed, the guest asks over the vsock channel the
agent already uses; a host-side component enforces a reviewed allowlist and size
and time bounds, and every fetch is recorded in the job's receipt. The guest
never receives an IP path.

**Rationale:** it keeps the isolation claim's network part intact, reuses the one
existing channel and its bounds, and leaves an auditable record instead of a
general-purpose route.

### Decision: name the escape-suite targets as the change trigger

**Choice:** the subsection names the suite's TCP targets (`1.1.1.1:443`,
`10.0.2.2:80`, `169.254.169.254:80`) and its vsock probes as the check that must
change in the same change that adds guest networking, together with the isolation
claim.

**Rationale:** the claim is only as strong as the suite that tests it; naming the
targets makes the coupling explicit rather than conventional.

### Decision: rejected alternatives

**Choice:** a guest NIC on a host bridge or NAT, and an in-guest or host-terminated
L3 tunnel (including a userspace WireGuard implementation such as sanstun) are
rejected because they give the guest general IP reachability and would require the
escape suite to stop asserting no IP egress.

**Rationale:** Kiln has no tunnel to replace and no need for one; a tunnel would
change the security boundary without changing the requirement.

## Risks / Trade-offs

- The subsection adds a maintenance obligation: a future egress change must update
  the claim and the suite together. That obligation is the point.
- Naming specific suite targets couples the prose to the test; a suite change
  should update the prose, which the requirement makes explicit.
