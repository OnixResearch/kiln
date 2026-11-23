## Design

### Profile selection (C1)

`kiln.implementation-authority-config.v1` has three profiles:

- `absent` denies every protected operation;
- `local_operator { operator }` is the explicit compatibility profile and never
  authorizes projection-enabled routes;
- `verified_ucan` names the audience, generation, packaged or file policy (with
  its BLAKE3), trusted roots, static Ed25519 keys, a bounded proofs directory,
  the revocation feed and its maximum age, replay retention and one holder seed
  per role.

The Nickel contract and the Rust loader enforce the same named bounds. Shells
resolve `--local-operator` or `--authority-config` (mutually exclusive) through
`ImplementationAuthorityConfig::resolve` and select it with
`Implementer::select_authority`; neither flag selects `absent`. Selecting the
verified profile chooses, for each role holder, exactly one grant addressed to
it that verifies now; an expired or untrusted grant is never selected.

### Payload v2 (C2)

```text
BLAKE3("kiln.implementation-authority-payload.v2",
       op, incarnation | "pre-launch", effect, expected_target, len(parts), parts…)
```

Every field is length-framed. The incarnation is the BLAKE3 identity of the
attempt's recorded `TransportLaunch`. The effect identity is the stable
reservation or effect id, so a fresh invocation of the same effect recomputes
the same digest and differs only in its holder nonce. Publication binds the
branch and plan; integration binds the base ref and merge commit.

### Durable authority state (C3)

Tables `authority_replay`, `authority_revocation`,
`authority_revocation_feed_view` and `authority_decision` as specified in C3.
`AuthorityStateStore` exposes `revoke`, `revocation_generation` (the highest
generation any feed view or revocation recorded) and `decisions` for S5 and
S6.

**Deviation: a separate authority database.** The plan placed these tables in
the ledger file. They live instead in `<ledger>.authority`, beside it.
Dispatch admission runs inside `admission_apply`'s `IMMEDIATE` transaction on
the ledger connection, and Basalt consumes replay as the final step of
verification. A replay insert on a second connection to the same file would
wait on the ledger's own writer lock and fail with `SQLITE_BUSY`; reusing the
ledger connection would nest transactions. A separate file keeps replay
consumption in its own immediate transaction, as Basalt requires, without
changing the ledger's transaction structure. The cost is that a replay row and
the ledger mutation it admits are not one atomic commit. A crash between them
leaves a consumed nonce with no effect, which denies a retry of that exact
invocation but not a fresh one. That is the same outcome as a lost response.

`RecordingAuthority` persists every decision. An allowed decision that cannot
be persisted closes its session and denies.

Decisions read the shell clock to the whole second, the resolution of UCAN
verification time. The revocation view is checked at the last millisecond of
the decision's second, so its maximum age holds at every instant that second
can stand for, and a feed observed earlier in that second is not dated after
the decision. A `max_age_ms` below one second cannot be enforced at that
resolution and is refused by the Nickel contract and the Rust loader. The
view's age runs from the feed's own `observed_at_ms`; importing never
refreshes it, and a feed is validated whole and imported in one transaction.

**One view per configured feed.** Several verified configurations can share
one ledger, and so one `<ledger>.authority`, with different feed files. Each
feed keeps its own `(generation, observed_at_ms)` row in
`authority_revocation_feed_view`, keyed by its configured path: absolute UTF-8
with no `.` or `..` component, rebuilt from its components so repeated `/`
normalize away. The key is never the resolved file and never anything the
feed's content claims, and the import reads from exactly that path, so no
filesystem lookup separates the key from the bytes read under it.
Intermediate symlinked directories are followed when reading; a
final-component symlink is refused. Canonicalizing would only merge two
spellings of one file, which as separate rows each must be fresh on their own
and so fail closed; a producer that re-points a symlinked directory keeps the
configured feed's row, where a resolved key would leave a long-lived authority
checking the old target. Selection records the feed and imports it but checks
no freshness: a stale feed still selects and its decisions deny. Each
protected decision checks freshness against the authority's own feed's row
only, so a stale feed never borrows another feed's freshness, and an authority
whose feed ages out denies however recently another feed was imported. A row
only moves to a newer pair, compared generation first inside the import
transaction: within one generation it never moves to an earlier observation,
so a process that imports a stale copy of the same feed cannot roll back
verified sessions that share the row. A higher generation with an earlier
time does move the time back, which fails closed. Revocations stay one shared
union across every feed and are only ever added. S1 introduced this schema,
so no earlier `<ledger>.authority` file on Kiln `main` needs migrating.

**Non-claim: feed freshness is trusted to its producer.** `observed_at_ms` is
an unauthenticated claim of the configured feed producer. A backdated
observation cannot extend freshness: within a generation it is ignored when a
fresher one exists, and an authority whose own feed's newest observation is
older than `max_age_ms` denies. A postdated time, up to the importing
process's clock, cannot be detected by Kiln, so a producer that dates a stale
revocation state as current extends its freshness. Kiln trusts the producer
for freshness; removing that trust needs an authenticated observation source,
such as a signed, timestamped revocation feed. The shared revocation union
lets any configured feed deny more, never less.

**Non-claim: one mount namespace.** A row identifies a configured path within
one mount namespace, not a file across namespaces. Processes sharing a ledger
must see the same feed files at the same configured paths.

### Holder signing

`RoleSigner` holds one `HolderKey` per loaded `GrantRole` and signs only the
operations its role owns. A coordinator key cannot sign verification,
publication, integration or lifecycle. Seeds come from operator files with
owner-only permissions and never appear in argv, logs or decisions.

**Seeds scoped to the entry point.** `select_authority` takes the holder roles
of the calling entry point and opens only those seed files; the signer holds
no other key. The set is fixed by the entry point, never by a flag:

| Entry point | Holder roles |
|---|---|
| Lattice worker, `kiln-lattice-dispatch`, fleet | Coding |
| `kiln-implementer` subcommand | the roles of its own `Protected`/`Reconcile` rows (`entry_point_roles`); `capture-candidate` is Coding, `publish` Publication, `status` none |
| `verify-implementation` | Verification |
| campaign drive | Coding at its claim, Publication at native delivery |

`admit_with` refuses an operation whose role this process did not load as
`authority: holder-not-loaded`, before anything is signed, sent to the
verifier, recorded or mutated, and before any credential use. A coverage test
checks that every CLI subcommand loads the role of every check its handler
reaches. The absent and local-operator profiles load no seed.

The plan's #69 acceptance row 5 ("broad verifier separate from narrow worker")
is met for the worker against the verifier: the Lattice worker and the fleet
load the coordinator seed only, and the verifier runs with only its own. It is
not met in full. In campaign deployments the drive's launcher runs the capture
role under the verifier's UID, so that UID holds both the coordinator and the
verifier seeds. Giving capture its own UID needs a candidate-handoff design
(group-readable staging and integrity between capture and verification);
that is a residual for S7's verifier separation.

### Entry-point coverage (C4)

`EntryPoint { path, item, class }` with
`EntryClass = Protected(op) | Reconcile(op) | ObservationOnly | OperatorConfig`.
An entry point that multiplexes commands (`admission_apply`, the `admission`
CLI command, native Git consumption) lists one row per class it reaches, and
`validate_entry_points` rejects identical rows. `ClaimRenew`'s owner path is
corrected to `lib.rs`, where `renew` enforces it.

`tests/authority_coverage.rs` scans the sources, ignoring comments and string
literals, and fails when:

- an `Implementer` public function, CLI subcommand, fleet route or campaign
  control route has no row, or a row names none;
- a `Protected(op)` row's code does not reach a `ProtectedOperation::op` check,
  directly or through crate functions and `Implementer` methods;
- a body checks an operation directly that none of its rows names.

### Claim path

`claim_next_deferred` claims inside `with_claim_ledgers`. That helper now
lends the authority slot and profile to its closure. The claimed attempt is
admitted with `admission::authorize_claim`, the same check admission claims
use, before the transaction commits. A denial returns the refusal and commits
nothing.

### Dispatch path

Every reservation and launch record admits `Dispatch` before it is saved:

- `reserve_dispatch`, `reserve_dispatch_plan` and `start_execution` run in
  `with_dispatch_ledgers`, which lends the authority slot, profile, last
  session and the recorded transport launches to the transaction. The attempt
  ledger applies its fence and phase checks to the transaction's copy first;
  `authority_bridge::authorize_dispatch` then admits the exact request for the
  attempt as it will be saved (the reservation's effect identity, its recorded
  incarnation, the plan and provider, or the plan and workflow for
  `start_execution`). A denial returns before the ledgers are saved.
- The fleet launch: `fleet_bind_session` admits the launch request (effect,
  plan, machine and agent kind) in the transaction that saves the `Placing`
  session, its durable launch record, after the fence, work and live-session
  refusals. The tick calls `fleet_authorize_launch` again immediately before
  creating the worktree and starting the credentialed agent; a denial there
  leaves the session placing and starts nothing.
- The GitHub CLI `reserve`, `reserve-plan` and `start` reach these methods
  through `open_implementer`; with no selection flag they run under the absent
  profile and are refused.
- The web campaign drive launches its worker, capture and verifier roles with
  exactly the authority source it selected itself: the same
  `--local-operator` or `--authority-config`, or neither for the absent
  profile. Each role resolves it through the same helper, so it decides on
  the drive's profile and never a wider one. Previously the roles received no
  source and always ran under the absent profile, so an authorized campaign
  dispatch could never reach Lattice. Under the verified profile each role uid
  must be able to read the configuration, the proofs, the revocation feed,
  `<ledger>.authority` and its own holder seed only (see "Seeds scoped to the
  entry point").
- `admission_before_transport` keeps its transport-time recheck: it is the
  currentness check at the effect.

**Credentials after authority.** The Lattice worker reads its one credential
variable before any durable step, so a refused credential leaves no
reservation. It first calls `authorize_dispatch_plan`, which admits the exact
request `reserve_dispatch_plan` will make on a read-only copy of the ledgers
and saves nothing. The order is pre-check, credential read, reservation
(checked), transport (rechecked). When the last admitted session is for the
identical request, `authorize_dispatch` revalidates it instead of admitting a
second grant, so the pre-check and the reservation consume one invocation.
The fleet and CLI paths read no credential themselves; the fleet's credentialed
agent starts only after the bind and the launch recheck.

`fail`, `block` and the other worker-lifecycle records stay
`Reconcile(Dispatch)`: fence-checked records of the dispatched run's outcome.
`prepare_native_git` stays `OperatorConfig`.

### Ledger readers

The `ObservationOnly` readers built on `with_ledgers` (`work`, `attempts`,
`attempt`, `plan_feedback`, `authorize_current`, `active_work`,
`dispatch_plan`, `dispatch_requires_recovery`, `dispatch_reservation`,
`swarm_snapshots`, `metrics` and the receipt readers) save the work and attempt
documents back in an immediate transaction and bump their row versions. Their
closures call only `&self` ledger methods, so within one binary the write
re-serializes the loaded values: attempt phase, lease, fence and claims are
unchanged, and authority decisions live in `<ledger>.authority`, which these
readers never open. A version bump changes `fleet_observation_version`, nothing
authority-relevant. An older binary's reader could drop fields its schema does
not know when it re-saves, the hazard that already keeps transport launches in
their own document. The readers are unchanged here.
