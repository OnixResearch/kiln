# Migration bridge plan

`kiln-microvm::bridge` is the pure admission layer for one VM-specific
Cloud Hypervisor migration bridge over Iroh. It binds the exact migration
operation, session, attempt/fence, checkpoint manifest identity, source and
target placement, authenticated endpoint public keys, protected local Unix
sockets, protocol generation and bounded stream resources.

Admission requires precopy mode, one connection, nonzero bounds and exact
matching of every independently resolved fact. Zero endpoints, equal endpoints,
same-host/non-advancing placements, unsafe or non-absolute sockets, postcopy or
memfd modes, wrong peers and wrong protocol generations are refused.

## Bounded Iroh shell

`kiln-adapter-iroh::migration` implements the VM-specific byte bridge, separately
from the unchanged bounded workflow exchange. `DirectMigrationEndpoint` uses the
existing Iroh **1.1.0** dependency, disables relays and address lookup, requires
the configured address, and binds a loopback address for the other family only
when the host has it, as 0.95.1 did per family. The endpoint alone produces a
`MigrationChannel` — a connection with one bidirectional stream whose fields are
private — so a caller cannot pair a stream with a connection or endpoint that
did not produce it. Admission checks the exact endpoint id, ALPN, authenticated
peer, 0-RTT state and all-direct selected path before any protected resource or
reservation is touched. One bidirectional stream has bounded flow-control
windows, no unidirectional streams and no datagrams.

`bridge` accepts only a fully established `Connection` and rejects 0-RTT stream
data, wrong negotiated ALPN (`kiln/ch-migration/precopy/1`), wrong actual
authenticated peer, wrong local EndpointId, stale plan/placement/protocol and
initial cancellation before protected-resource access. A fixed bounded header
binds the actual stream to the canonical operation/plan identity before CH bytes
flow. The provider must durably consume that exact operation once; endpoint or
controller restart cannot blindly replay its bytes.

An independently supplied `MigrationReservation` must retain current operation,
placement and resource authority. It resolves the exact expected plan and local
VMM UID/PID. Both the private socket directory and actual Unix peer credentials
are checked; unrelated same-user processes cannot masquerade as that VM. A
source listener is never unlinked/reused automatically, and destination access
is restricted to its exact service-owned socket. No generic remote command or
arbitrary CH API route is exposed.

Each direction uses a fixed 64 KiB buffer and awaits downstream writes for
backpressure. Both directions share total-byte and bandwidth budgets. EOF
half-closes only the corresponding writer, while reset, deadline, cancellation,
revocation or socket failure records an uncertain consumed operation and resets
the stream. Currentness/path is rechecked during copying. Relay fallback cannot
silently carry bulk RAM: the endpoint builder disables it. The receipt records
actual initial/final direct addresses, byte counts and elapsed stream duration;
it explicitly does not commit placement. This is not guest TCP-continuity or
downtime evidence.

## Selected Aspen prerequisite, not a pin substitution

The actual selected host runtime manifest/lock names Molten
`22f8ded26ca1907c29948e08b53f35df23080733` and Iroh **1.1.0**. It does not select
the older issue's inspected `77aa73a2` cohort or the current sibling checkout by
implication. Inspection of the exact cached selected Molten source shows:

- `fabric_transport/cross_process/iroh_shell.rs` listener/client validation still
  requires loopback binds;
- `accept_one_derived_frame` exchanges one bounded frame, not an admitted raw CH
  migration stream;
- actual `remote_id()` is recorded, but its Established peer-context field is
  copied from the expected descriptor, not resolved from actual authentication;
- Kiln's selected `AspenServiceTransport` consumer exposes trigger ingress and
  reconciliation, not linearizable VM placement or physical volume/effect fencing.

Consequently the selected hosted authority refuses retained/migration mutations
before host effects. No local marker, stream receipt or operator version label
substitutes for those producer mechanisms. Aspen #15 must supply the admitted
registered cross-host profile with actual peer-context binding and durable
operation evidence; placement/resource authority needs its qualified owner API.
The workflow transport and the host runtime now both pin Iroh 1.1.0; sharing a
version is not a qualified composition, which still needs its own evidence.

## Coordinated checks

Parent runs `cargo test --locked -p kiln-adapter-iroh migration`,
`cargo test --locked -p kiln-microvm`, and
`cargo test --locked -p kiln-hosted --lib microvm` after integration. These rails
exercise bounded copy/half-close/budget behavior, pure admission, durable local
storage, paired integrity, journal replay and negative provider admission; none
is two-host CH qualification.

Required operator qualification needs two compatible KVM hosts, exact admitted
CH/CPU/device/memory-backing/image/workspace identities, the real Aspen service
and volume/effect fencing owner, and the retained Lattice guest/reconnect
gateway. Exercise A-to-B-to-A with process counter plus durable checksummed
workspace/log cursor, busy RAM/disk, ordinary nonzero sender exit after lost ACK,
receiver writes then crash, endpoint/controller restart, old-host reboot,
backpressure, cancellation and source-retirement uncertainty. Record stream
transfer, guest pause and application reconnect separately. No live mode is
enabled without final-ACK-safe actual resource behavior and qualification.

## Non-claims

The pure plan module opens no socket. The new Iroh shell performs real bounded
byte forwarding only under an independently supplied admitted reservation; the
selected Aspen runtime does not yet provide it. No two-host migration, relay
profile, guest Ethernet/TCP continuity, durable placement commit or live-drain
qualification is claimed. Workflow exchange and existing no-device CI/static
guest/legacy tap0 policies remain unchanged. Maintenance owner is Kiln's Iroh
VM adapter; Aspen owns admitted fabric mechanisms and onix-core owns deployment.
