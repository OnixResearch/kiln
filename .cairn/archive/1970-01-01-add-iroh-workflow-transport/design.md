## Context

`LatticeAdapter<UnixWorkflowExchange>` already lowers and admits Kiln workflows, with native producer framing and recovery. Both GitHub and Radicle shells configure its Unix endpoint. The separate Iroh observation plan grants no execution authority and is unchanged.

## Decisions

### Decision: Transport the existing native contract

**Choice:** Add a controller-local Unix proxy and an Iroh worker bridge to one operator-selected local Lattice socket. Exchange the existing `ExchangeRequest`/`ExchangeResponse` under `kiln.workflow.v1`, one request per connection. Keep all caller APIs and native identities unchanged.

**Rationale:** Existing consumers adopt remote execution by changing only their socket configuration. Lattice retains execution, durable acceptance and recovery; Kiln retains all CI-state/outcome decisions. No provider implementation or async type enters core crates.

### Decision: Explicit stable identities and bounded effects

**Choice:** Pin Iroh 0.95.1 (MSRV 1.85, compatible with the repository's Rust 1.88), disable ambient discovery and default relays, accept optional explicit HTTPS relay URLs, load protected stable secret-key files, pin the remote endpoint key, and require an explicit worker peer allowlist. Admit exact native contract profiles before opening the worker socket. The controller Unix socket is private and checks declared OS user IDs.

**Rationale:** Dial hints and relay locations are not authority. Peer enrollment is explicit execution authority; an authenticated but unlisted peer cannot reach Lattice. Profile admission is pure; endpoint/files/socket operations remain in shells.

### Decision: Fail unknown after possible acceptance

**Choice:** Apply finite frame/log/concurrency/deadline bounds at both bridges. Validate native request and response identities and linkage. On transport, provider or timeout ambiguity, close the stream without synthesizing a successful or retryable reply. The existing Unix consumer consequently reports UnknownAfterWrite and uses RecoverDispatch, never blind redispatch.

**Rationale:** A deadline or lost response cannot establish that execution did not start. The bridge retains no execution ledger and cannot replace Lattice's durable record. Restart recovery sends the original exact request.

## Risks / Trade-offs

- The local proxy adds a process/socket hop but avoids changing every workflow consumer or inventing a second wire/error contract.
- Static enrollment is an execution grant for the selected contract profile, not source trust or confinement. Removing a peer requires reloading/restarting the immutable profile; it does not cancel already accepted work.
- Local tests exercise real authenticated QUIC over loopback, not public NAT traversal or relay availability. Relays remain explicit operator-controlled infrastructure.
- An ambiguous exchange stays unknown even for failures that may have happened before dispatch; conservative recovery is safer than accidental duplicate execution.
- Production isolation, secret provisioning, durable Lattice runtime configuration and deployment remain operator responsibilities. This change authorizes no remote mutation or lifecycle archival.
