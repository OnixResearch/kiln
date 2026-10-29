## Context

The current service is single tenant and exposes HTTPS. Native Iroh transports already exist for workflow and observation contracts. GitHub CI is a separate admitted composition with exact source acquisition, durable replay and commit-status publication. Hosted customer authority, finance and isolation are distinct from those mechanisms.

## Goals and Success Contract

Close the nine reviewed gaps through an end-to-end Iroh-connected hosted composition, retaining independent qualification obligations and no production claims without observed evidence. Customers keep source upstream. Existing pure CI core, exact identities and recovery semantics remain intact.

## Decisions

### Decision: Iroh is customer transport, not authorization

Choice: use the pinned Iroh cohort, a distinct versioned customer ALPN and bounded framed requests. Authenticate peers through QUIC; resolve organization membership and current scopes server-side. Address hints and relay reachability grant no authority. A local loopback bridge serves the browser; no public browser bearer token or caller-supplied forwarding identity is trusted. Source-provider webhooks/checkout/status retain native protocols.

Rationale: browsers cannot directly use the existing native QUIC client and transport authentication alone cannot enforce tenant authorization. Do not reuse workflow ALPN for general administration.

### Decision: Hosted composition outside CI core

Choice: add a separate hosted composition reusing existing adapter ports and web functionality through reviewed private bindings. Organization, connection, pricing and operational decisions stay outside kiln-core. Durability precedes external effects, uncertain effects remain reserved, and all mutable operations use request identities and current revision checks.

Rationale: a new UI or transport must not create a second CI state machine or erase replay protection.

Invitation acceptance commits membership, invitation consumption, successful
replay receipt and audit in one SQLite transaction. Any rejection or receipt
failure rolls back the whole acceptance, so an unaffiliated peer cannot spend
permanent global replay capacity. Issuer discovery/JWKS are current authorized
reads outside the effect-replay ledger; successful and uncertain effects retain
their existing replay protection. Neither path prunes retained obligations.

### Decision: Accountable implementation boundaries

Choice: independently implement accounts/connections, finance, pipeline compilation/migration, execution/fleet, secrets/workload identity and operations. Each slice must have a real callable shell and integration consumer, not an unused model. Parent owns manifests, common dispatch, Iroh and browser integration. Runtime profiles carry exact backend inputs; unsupported provider features fail before effects. No shims implying unsupported Actions compatibility.

Rationale: independent bounded modules permit parallel delivery without conflicting state ownership. Reserve-before-effect ordering crosses module boundaries explicitly; no unknown reservation is freed automatically.

## Risks / Trade-offs

- Hostile customer execution requires an actual isolated backend and adversarial observations; namespace flags alone do not qualify it.
- Cloud issuer trust, merchant eligibility, approved prices, supplier capacity, production TLS/relay/secret configuration and independent security review require explicit operator inputs. Local engineering cannot establish them.
- Actions migration must reject unsupported actions/expressions, never silently drop behavior.
- A private per-organization deployment bounds initial control/data scope; it does not remove authentication, scoped authorization or secret separation requirements.
- Existing release holds and changes remain intact. No publication, deployment, live source-host mutation or financial action is authorized.

## Verification Strategy

Validate native proposal/design/task gates before implementation. Run focused positive/negative behavior checks once after integration, plus actual Iroh and browser smoke, remote-source CI path, process teardown, finance concurrency/restart, pipeline/migration input boundaries, secrets expiry/revocation and backup/restore. Retain observed commands and receipts; report unavailable operational prerequisites without inventing outcomes.
