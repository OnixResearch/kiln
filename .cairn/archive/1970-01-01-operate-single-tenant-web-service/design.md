## Context

Existing HTTPS observation deliberately has no control intake; the offline
catalog deliberately has no backend. Core commands, canonical outcomes, Git
snapshots, Lattice workflow lifecycle/controls/log reads and bounded child
execution already exist. Integration belongs to a thin application shell, not
another CI state machine or an extension of observation authority.

## Decisions

### Decision: One explicit single-tenant application composition

**Choice:** Add `kiln-web-service` with an allowlisted typed project catalog,
registered HTTPS origin, separate operator/worker token references and positive
resource bounds. Browser enrollment selects installed profiles, never arbitrary
paths, commands, providers or routes. Resolve the declared Git reference once
through the existing adapter and bind the exact revision/tree and reviewed
pipeline to the persisted workflow plan.

**Rationale:** This provides the concrete Kiln self-CI consumer without implying
multiuser hosting, global trust or an unreviewed repository execution service.

### Decision: Reuse canonical CI and native provider transitions

**Choice:** Reuse core `decide`/`evolve`, workflow admission and the maintained
Lattice adapter. SQLite WAL/FULL persists configuration identity, enrollment,
idempotent request reservations, canonical events and provider observations
before effects. Restart recovers original dispatch; uncertainty remains unknown.
Only one execution per project may remain active or unresolved. Controls bind
current canonical revision, attempt, native execution and provider sequence;
unsupported/stale controls fail before effects. Ambiguous controls are observed,
not automatically repeated.

**Rationale:** Web requests are a new intake surface, not CI outcome authority.

### Decision: Distinguish native diagnostics and captured worker output

**Choice:** Preserve actual Lattice diagnostic logs and their native cursor/gap
facts. The maintained producer retains terminal diagnostics, not command output.
A fixed-profile local Git job verifies source context, materializes an admitted
exact checkout and uses existing Bounded Exec. Its separate worker credential
may locate only its source-bound active run and upload bounded captured stdout,
stderr and real artifact bytes for the matching attempt/execution/node. Captured
output uses an explicitly worker-owned byte cursor, never a claimed native
provider cursor. Atomic retained bytes are BLAKE3 checked and exposed under
opaque IDs, never caller filesystem paths. Duplicate identical uploads are
idempotent; conflicting or stale bindings fail before publication.

**Rationale:** Current workflow observations carry terminal diagnostic and receipt
references, not arbitrary command output or product-byte transport. Narrow worker
ingress avoids inventing logs, relabeling receipts as products or attaching old
files to a new run.

### Decision: Separate browser control authority and preserve offline mode

**Choice:** Serve a dedicated live interface using service-specific HTTPS bearer
authentication. Tokens remain memory-only in the browser, distinct from worker
credentials and read-only observation credentials. Registered Host/Origin,
bounded HTTP framing/connections/deadlines, no permissive CORS and durable
idempotency guard mutation. Existing offline HTML and its tests retain their
read-only contract. Provider data is rendered as inert text.

**Rationale:** A simple single-owner service does not require inventing accounts,
SSO, cookie sessions or browser private-key provisioning.

## Risks / Trade-offs

- Repository commands are trusted, operator-reviewed workloads. A temporary
  checkout and Bounded Exec do not prove hostile-code confinement. Worker
  placement/isolation remain provider/operator responsibilities.
- Trusted worker artifact declarations bind exact admitted identities and bytes;
  they do not prove product semantics or arbitrary provider honesty.
- HTTPS setup requires operator-owned certificates and private credentials.
  Isolated API/worker checks validate an explicit test CA; the owned browser pins
  the exact isolated server leaf SPKI rather than disabling certificate checks globally.
- Finite bounded live streams reconnect and may report gaps; they do not imply
  complete delivery. No fallback executor or inferred success is permitted.
- The selected native Lattice cohort accepts cancel but its completion classifier
  always supplies `cancelled: false`, so observed cancellation becomes
  `DomainFailure`. The strict native `Cancelled` assertion remains failing.
  Correcting and qualifying a different producer cohort is separate from
  relabeling observations or claiming this service is qualified.
- Local smoke uses disposable state, sources and native workflow resources. No
  production activation, forge/status publication, lifecycle acceptance/archive
  or source push is authorized by this implementation.
