## Why

Kiln has separately exercised its GitHub CI shell and Iroh workflow transport, but has not recorded a genuine combined GitHub delivery, remote build/test, canonical outcome, and GitHub commit status. The current consumer is this repository's operator-controlled CI validation branch. Kiln maintainers own the composition and repeatability rail; the repository operator owns credentials, ingress, enrolled hosts, and workload authority.

## What Changes

- Add a repeatable operator-input composition and live exercise using normal Kiln binaries, SecretSpec 0.19.0, the reviewed Lattice runtime, and two distinct machines.
- Preserve the existing GitHub event, exact-source helper, Lattice native exchange, Iroh peer admission, SQLite replay, and status reconciliation contracts; introduce no fallback executor or fixture transport.
- Select the existing typed loopback listener on a separate controller host where its default port is unoccupied, leaving unrelated ingress handlers and the configuration contract unchanged.
- Execute a passing build/test, a genuinely failing build/test, and an interrupted-controller/bridge recovery while the admitted remote job continues. Require matching real GitHub terminal status observations, not just a local outcome.
- Retain exact reviewed inputs, executable/source identities, forge delivery and status identities, worker execution facts, and scoped verification evidence. Update operator instructions for reproduction.

## Impact

- **Files**: this change package; a repository-owned live composition/exercise shell and reviewed typed inputs; operator documentation; change-local nonsecret evidence.
- **External effects**: a dedicated GitHub validation ref and one narrowly selected webhook; one new HTTPS ingress endpoint; isolated controller/worker processes, private state, and Iroh identities. Do not alter `main`, repository governance, existing Radicle CI, existing ingress handlers, unrelated services, or production state.
- **Testing**: exercise normal binaries across actual distinct hosts with genuine GitHub intake/acquisition/status writes; verify success, nonzero test failure, replay deduplication, and interrupted controller/bridge recovery. Run affected positive/negative checks and required native Cairn gates.
- **Non-goals**: arbitrary untrusted workload confinement, automatic recovery of a killed Lattice executor, NAT/relay/WAN guarantees, indefinite availability, credential issuance, release readiness, or remote source publication.
