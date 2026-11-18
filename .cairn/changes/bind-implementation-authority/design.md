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

Kiln's vendored UCAN `d46816f` is an ancestor of Basalt's `c483c7b`, but the two
snapshots are not interchangeable under Kiln's current toolchain. Attempting the
upgrade showed that `c483c7b` adds a durable-authority module whose
`replay-ledger-core` and `revocation-view-core` dependencies at `a0de7930`
require rustc 1.96, while Kiln pins Rust 1.88.0. Cargo cannot resolve two
same-name, same-version path packages, so a second vendored UCAN copy is not an
option either.

One cohort therefore requires one of:

1. Upgrade Kiln's Rust pin to 1.96 and import the two additional pinned sources,
   then import Basalt with a repo-owned script that omits `vendor/` and rewrites
   its three UCAN path dependencies to Kiln's single `vendor/ucan*` copy.
2. Consume Basalt from a source-distinct (git) dependency and keep two cohorts,
   accepting the Nix prefetch and dual-verification-profile cost.
3. Keep Basalt out of process, which cannot return the live session this design
   requires.

Option 1 is preferred: one cohort, an additive upstream delta, and no duplicate
UCAN crate. It is a toolchain change with its own validation, so it must be an
explicit decision rather than a side effect of this feature. Until it lands, this
change stops after the coverage contract; the authority port and enforcement
tasks stay open.

Basalt stays AGPL-3.0-or-later; `LICENSES.md` will record the linked cohort.

### Non-claims

Authority proves permission for one exact request, not effect completion,
independent acceptance, confinement, resource release or release eligibility.
A retained session, receipt or projection cannot authorize another request.
