# Design: Distinguish runnable Celld Worker evidence from model evidence

## Existing source and owners

`kiln-adapter-celld/src/admission.rs`, `partition.rs`, `profile.rs`, and `protocol.rs` own pure admission, BLAKE3 partitioning, cohort declarations, and HMAC signing. `worker.rs` owns an explicitly in-memory `BTreeMap` model; its restart fixture copies model state, not SQLite or a celld node. `client.rs` owns `EventStorePort`, but currently returns an empty snapshot from `load` and misclassifies a lost response as retryable. `config/celld-wrangler.ncl` names missing `src/index.ts`; the Kiln flake's `celld-cohort` output is a verified v0.2.1 *source archive*, not an executable. The host executable is 0.3.0, not the admitted cohort.

The previously observed global coverage result listed all 22 accepted `kiln.celld.*.scenario.*` identities as missing and `kiln.celld.worker`, `kiln.celld.error_ownership`, `kiln.celld.composition_root` as dangling. Do not rerun that known failure to confirm it. Native policy `cairn-policy/default.ncl` checks exact markers only in maintained `crates/**/*.rs|*.ncl` and `tools/`, so source comments alone cannot establish semantics. No weakening of accepted scenarios or policy is permitted.

## Scenario evidence inventory and intended owner

| Accepted scenario IDs | Baseline path / evidence gap before this change |
|---|---|
| `dependency_boundary.scenario.optional`, `.import` | Optional core build and conformance dependency boundaries; need actual import-denial evidence, not source-string tests. |
| `cohort.scenario.valid`, `.output_gate` | `admit_profile` accepts static declared cohort and rejects disabled output gate; no observed v0.2.1 binary/deployment/probe yet. |
| `partitioning.scenario.stable`, `.cross_partition` | `map_partition`, `admit_append_batch` contiguous single-stream admission; assert actual mismatched stream is rejected without writes. |
| `atomic_append.scenario.commit`, `.interrupt` | Model stores events/outbox in memory; implement real Worker SQL transaction and rollback fault test before linking these IDs. |
| `transport.scenario.valid`, `.rejected` | HMAC pure Rust and model tests exist; executable Worker must verify an actual signed request before SQLite, reject private routes and nonce/signature changes. |
| `ambiguity.scenario.repeat`, `.disconnect` | Model stores an operation map; Worker needs persisted idempotency and Rust client must classify disconnected outcome `Unknown` rather than `Retryable`. |
| `replay.scenario.valid`, `.damage` | `replay_checked` calls production evolve after identity validation; add real positive/negative journal data and Worker corruption detection. |
| `leases.scenario.restart`, `.ambiguous` | `lease_recovery_action` preserves ambiguity; model restart is a clone, not node takeover. Local SQL reopen is evidence only for persistent fields, not fleet restart. |
| `migrations.scenario.valid`, `.mixed` | Pure `plan_migration` validates edges; Worker must transact actual schema before claiming valid migration. Profile drift can block an incompatible cohort. |
| `authority.scenario.tenant`, `.operator_api` | Pure trust-domain rejection exists; add actual private route denial before dispatch. |
| `conformance.scenario.wrong_boundary` | Authentication fixture must fail on signature mismatch with reachable healthy SQLite, not on missing runtime. |
| `nonclaims.scenario.acknowledged` | Evidence wording and receipts must limit local observations; a model acknowledgement cannot prove a deployed fleet. |

## Runtime acquisition and local contract

The pinned celld v0.2.1 tar is available via Kiln's flake and contains Durable Object examples and a `transactionSync` harness. Nixpkgs advertises `deno`, `esbuild`, `wrangler`, `garage`, `s3proxy`, and `sqlite`, but only `esbuild`/`sqlite` were confirmed locally realized at proposal time; `workerd` is not a direct package. MinIO is marked insecure in the locked nixpkgs. Try safe offline alternatives and an exact-cohort executable before claiming lack of a runnable celld fleet. A local JS Worker test may exercise the identical SQL and request code using Node's real SQLite driver, but MUST label itself local and cannot verify output gating, probe, node ownership, or fleet durability. The worker artifact must not depend on the in-memory Rust model for its implementation.

## Expanded pinned-cohort and conditional-control scope

The repo-owned `tests/celld-live-cohort.sh` must build the digest-checked v0.2.1
archive with its locked Cargo graph and the locked flake tools. It starts an
isolated SeaweedFS S3 backend whose conditional-write probe Celld checks,
deploys the exact owned Worker, and runs gated Celld behind a loopback Caddy
TLS endpoint with a temporary CA passed explicitly to Rust. Its cleanup trap
removes storage, certificate, secret, and service processes. Positive phases
run with `CELLD_OUTPUT_GATE=1`; a separate disabled-gate negative phase must
fail before canonical mutation. Restarts must read persisted facts through
signed HTTPS rather than reusing only local model state.

The trusted local operator selects a measured instance of that pinned
artifact, bundle, deployment version, storage cohort, and ingress CA.
Kiln-owned binary, bundle and CA content identities use separately framed
BLAKE3 domains; the upstream source archive retains its Nix SHA-256 SRI.
At each startup it independently captures the executable digest/platform,
Worker declaration, active gate/probe process settings and readiness, and
ingress origin. The adapter rejects any absent, contradictory, or stale
observation before dispatch. Successful TLS Worker responses must also
report the selected deployment version, revision, protocol and schema;
a contradiction is `Unknown`, not an accepted append. These are measured
loopback process facts, not signed Celld node attestation or authority to
admit an unobserved remote fleet.

The `celld-boundaries` Nix check executes core, provider and standalone tests
in a Celld-free dependency tree. In its disposable sandbox it also copies
the workspace, compiles an actual imported forbidden implementation type
into core, and requires the same Cargo-tree gate to reject and name the
new edge. The real checkout is never modified by that negative fixture.

The signed Worker `/v1/lease` route applies only an exact-current conditional
plan and persists owner, expiration, attempt, idempotency, completion, and
reconciliation facts in SQLite. Pure Kiln admission supplies time and lease
decisions; the Worker cannot dispatch providers. The signed `/v1/migrate`
route distinguishes virgin state from an explicitly initialized prior schema
zero, requires the stored BLAKE3 input identity and the admitted zero-to-one
edge, records provenance atomically, and rejects mixed Worker/schema
declarations before resolving an object. Node SQLite tests inspect conflict
and rollback boundaries; the pinned local runner checks restart and
mixed-cohort behavior with the production Rust HTTPS adapter.

The stack-owned sibling `bounded-http` supports scoped DER roots and bounded
HTTPS, but its current manifest requires Rust 1.96 and forbids publication,
while Kiln's supported workspace compiler is Rust 1.88 and no immutable
published dependency URL is available. Do not import a sibling filesystem
path or silently change Kiln's toolchain: isolate the concrete Celld HTTPS
shell behind the optional adapter, disable ambient proxies and redirects,
and retain explicit trust roots, deadlines, and redacted diagnostics.

## Implementation and verification

Expose an actual Kiln Worker JavaScript entry at the configured Wrangler path, using the pinned cohort's Durable Object SQL API and one transaction for events, outbox, revision, and operation result. Test SQL with a real temporary SQLite database and an injected precommit fault, close/reopen to check persistence, and authenticate both good and hostile requests. Align Rust ambiguous-result admission and fail closed on unsupported reads rather than returning a fabricated empty state. Only add scenario markers beside tests that exercise the exact accepted scenario, and keep non-exercised scenarios openly missing. Record exact local tool/source attempts and non-claims. Run focused Worker/adapter tests, affected workspace checks, native Cairn validation and gates; sync or archive only when global traceability is legitimately complete. r[kiln.celld.local_worker_evidence]
