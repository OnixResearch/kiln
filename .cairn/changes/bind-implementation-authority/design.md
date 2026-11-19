## Design

### Layer split

- `kiln-core::implementation::authority` owns the closed coverage map, the typed
  authority request/decision facts and the port trait. It stays `no_std`-friendly:
  no keys, clocks, processes or I/O.
- `kiln-adapter-basalt` implements the port over `basalt::kiln`. It holds the
  operator-loaded policy, generation, audience, trusted roots and an opaque live
  session. Shells own token acquisition, replay storage and revocation freshness.
- `kiln-implementer` calls the port before claims, dispatch, cancellation and
  publication effects; the operator CLI exposes bounded redacted decisions.

### Coverage map

`ProtectedOperation` mirrors `basalt::kiln::Operation` one-to-one. Each entry
names the Basalt operation id, the enforcement owner path, the effect class and
whether a candidate identity is bound. The map is a fixed array; a test asserts
every variant appears once, every owner is unique and the candidate rule matches
Basalt's (`submit`, `verification`, `publication`, `integration`, `lifecycle`
bind a candidate; claim, renew, dispatch and cancel do not).

A protected path with no entry denies. No operation implies another.

### Request binding

The adapter builds one request per protected operation from trusted owner facts:
holder `subject`, deployment `audience`, project, work and version, attempt and
fence, authority generation, canonical payload digest, target and the applicable
candidate digest. Basalt frames and checks these fields; the caller cannot supply
its own trusted current-state observation.

### Ingress and currentness

`verify` returns an opaque session only after signature, proof, holder, root,
policy, time, revocation and replay checks. `revalidate` runs before each
protected effect and performs no second replay commit. Kiln composes the session
with its existing claim, fence, work-version and resource admission; neither
check substitutes for the other.

Cancellation, supersession and fence replacement block later effects even when a
caller presents another still-valid proof for the old attempt. Required
currentness that is unavailable fails closed for new effects; independently
authorized reconciliation and cleanup remain possible.

### Separation

Claim, dispatch and candidate-submission authority cannot admit verification,
publication, integration or lifecycle completion. Each uses its own operation and
its own grant. Independent verification (#46) still decides acceptance; an
authorization decision creates no passing check.

### Compatibility

The local-operator profile records its provenance in the decision and is only
available to an explicitly configured local operator. It grants nothing by role
alone and does not apply to projection-enabled routes, which require an operator
capability. There is no missing-authority or alternate-provider fallback.

### Cohort and import

Basalt's reviewed snapshot is UCAN
`417f74662b9de2de3bdabe93ad5db996e1bd2861`; the earlier `c483c7b` drain tip is
its ancestor. Kiln imports that exact revision byte-for-byte and keeps one
snapshot for observation authentication and implementation authority.

The earlier blocked-prerequisite measurement assumed Basalt's default features.
The adapter depends on `basalt` with `default-features = false`, so
`oidc-exchange`, `durable-authority`, `bounded-http`, `replay-ledger-core` and
`revocation-view-core` never enter Kiln's resolved graph. The remaining
declaration blockers are handled at the consumer: `detect-coding-agent` 0.1.1 is
vendored from its exact registry archive with one recorded `rust-version`
declaration change (1.91.0 to 1.88.0), and Kiln's existing `uuid` 1.26.1 lock
entry satisfies SecretSpec's `uuid = "1"` requirement. No third-party source
code changes; the package metadata change is recorded in
`vendor/detect-coding-agent.source.json`.

Basalt is imported through the repo-owned pinned-source importer, which applies
three recorded path rewrites from Basalt's nested UCAN copy to Kiln's single
`vendor/ucan` snapshot and refuses drift on re-run. The nested snapshot,
release tooling, tests and examples are omitted. Basalt stays
AGPL-3.0-or-later; `LICENSES.md` records the linked cohort.

Rejected alternatives remain: upgrading Kiln's Rust pin, keeping two source
cohorts, or silently enabling Basalt defaults. None is selected.

### Non-claims

Authority proves permission for one exact request, not effect completion,
independent acceptance, confinement, resource release or release eligibility.
A retained session, receipt or projection cannot authorize another request.
