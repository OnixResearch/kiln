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
`authority_revocation_view` and `authority_decision` as specified in C3.
`AuthorityStateStore` exposes `revoke`, `revocation_generation` and
`decisions` for S5 and S6.

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

### Holder signing

`RoleSigner` holds one `HolderKey` per `GrantRole` and signs only the
operations its role owns. A coordinator key cannot sign verification,
publication, integration or lifecycle. Seeds come from operator files with
owner-only permissions and never appear in argv, logs or decisions.

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

### Classified gaps

These are recorded in the table for an owner decision, not wired here:

- The fleet tick launches a worker session after `fleet_bind_session` with no
  `Dispatch` admission. Its claim is `ClaimAcquire`-admitted.
- `reserve_dispatch` and `reserve_dispatch_plan` record durable intents
  without authority. Lattice admits `Dispatch` at
  `admission_before_transport`; the GitHub CLI's `reserve`/`start` path is
  operator-driven and records no `Dispatch` admission.
- `fail`, `block` and the worker-lifecycle records are classified as
  `Reconcile(Dispatch)`: fence-checked records of the dispatched run's outcome
  that present no fresh invocation.
