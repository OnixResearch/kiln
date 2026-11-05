# Native Radicle COB reduction and own reviews

Kiln appends only its explicitly declared `review` action to a declared
`xyz.radicle.patch`. The adapter now uses the isolated
`tools/radicle-patch` upstream reducer and writer, not a second permissive
in-process patch interpreter. Core crates do not link the Radicle provider.

The helper pins `radicle = 0.25.1` (crate SHA-256
`fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`).
Build it with the separately selected Rust 1.97 cohort described in its README;
the root workspace remains on its own toolchain. Configure its absolute path,
BLAKE3 and version in `RadicleMutationProfile.helper`. The Nickel profile's
all-`a` digest is synthetic configuration evidence, never an executable pin.

## Native read and authorization

`read` loads all patch namespace heads, admits a bounded reachable change DAG,
checks every change signature, then calls upstream `Patches::get`. The projection
contains the exact head set and native state, including revisions (id, base,
head, author), reviews, merges and lifecycle. `read-issue` does the same through
upstream `Issues::get`. `RadicleJobShell::read` uses the existing pinned
`xyz.radworks.job` action reducer after bounded signature admission. Job runs
remain owned by their signed actor: another actor cannot finish them.

Every native issue/patch namespace head must match its owner's verified signed refs and its tip's
signed author. Every visited branch must connect to the declared object root.
Opaque or unknown action encodings fail admission; recognized unauthorized
actions retain upstream's permission semantics. Complete authenticated heads are
compared again after reduction so state and preview cannot come from different
head snapshots. Job admission checks ODB commit/tree/blob headers before the
shared store can allocate action payloads.

Upstream identity documents and authorization decide which patch/issue actions
have meaning. In particular, non-delegate merges without a matching crefs allow
rule do not acquire meaning. A projection grants no Kiln review, CI, merge or
release authority. The optional raw web COB browser remains raw observation;
these verified reads do not silently upgrade its claims.

## Authority and current head-set binding

The existing `CollaborationMutationPort` flow remains:

```text
observe -> exact freshness/plan admission -> resolve own signer -> preview -> dispatch
```

The profile allowlists RID, patch object, typed revision-bound review action and
payload identity. The signer must equal the planned actor. Foreign authors and
concurrent heads are normal, not grounds for granting or denying authority.
Preview and dispatch compare the complete observed head set, and the helper
compares that set again at the actual signature boundary. Native reduction and
append run in a bounded mutable-ref snapshot sharing only the Git object DB.
Publication then uses conventional Git reference locks and expected-old CAS over
the signer's complete observed namespace before updating its COB and signed-ref
tips. A same-signer external writer cannot be force-overwritten; a cooperating
helper lock is only an additional optimization, not the correctness boundary.
Other namespace histories remain independent. There is no remote publication.

A head-set move requires a new preview. Kiln cannot create, close, archive,
merge or revise patches through this port. There is no raw-writer fallback.

## Idempotency, uncertainty and bounds

A matching own review of the exact revision, verdict and summary reconciles
without signing. An existing different own review fails closed rather than
pretending to replace upstream state. A child failure after possible write is
`Unknown`; reconciliation reads native state without acquiring a signer.

History defaults are 1,024 changes and 16,777,216 action bytes. The helper admits
at most 256 heads/parents, bounds its output to one MiB at the adapter, and runs
in Bounded Exec with a cleared environment, a 15-second process-group deadline
and bounded stdout/stderr. Operator history bounds are passed before reduction.
SecretSpec remains separately pinned; captured signer output and the retained
helper seed are zeroized, and the seed enters only the child environment, never
argv, configuration, observations or logs.

## Local behavioral proof

Run the isolated helper fixture (no remote operation and disposable keys only):

```console
cargo +stable test --locked --offline --manifest-path tools/radicle-patch/Cargo.toml -- --nocapture
```

The `concurrent_delegate_reviews_native_projection_append_and_stale_refusal`
fixture creates a real native repository/identity, adds a second delegate,
constructs concurrent reviews, compares the helper's native state with
heartwood, verifies a two-parent own review append, ignores an unauthorized
merge, and refuses an old preview. It replaces the former synthetic
single-writer fixture with an invalid RID and empty identity document. The
adapter's caller suites are run by integrated verification, not claimed by
this document. Local evidence never proves peer convergence, remote visibility,
source trust, patch correctness or release readiness.
