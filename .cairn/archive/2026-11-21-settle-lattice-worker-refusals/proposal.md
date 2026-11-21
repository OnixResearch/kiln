## Why

Kiln #45's worker transport (`kiln-lattice-dispatch --worker-request`,
`kiln_adapter_lattice::worker`) dispatches one admitted Lattice
implementation attempt. Lattice `db64cd65` (change
`execute-admitted-implementation-attempt`, `docs/candidate-worker.md`,
"Coordinator transport") now publishes the coordinator contract this
transport must consume, and the Lattice status comment on Kiln #45
(2026-11-21) lists five gaps in Kiln:

1. The prepared request does not carry Kiln's dispatch-plan identity, so the
   request bytes do not bind the plan Kiln reserved.
2. `kiln-implementation-fixture bind-execution` and
   `docs/implementation-runbook.md` still describe request schema v1, a shared
   network and `/run/netns/kiln-scripted-model`, which Lattice now refuses.
3. `config/implementation-worker.ncl` admits only transport
   `kiln.lattice-implementation-cli.v1`, while the campaign requires `v2`.
4. The transport clears the worker environment completely, so a model broker
   that names an `env` credential can never resolve it.
5. Any non-zero Lattice exit is retained as a possible dispatch, even a typed
   `lattice.implementation-attempt-refusal.v1` with `dispatched:false`; and
   recovery's `not_started_closed_by_recovery` is reconciled as if it were an
   execution.

## What Changes

- Campaign binding prepares the native request once to learn its worker
  profile, computes the dispatch-plan digest, inserts `dispatch_plan` into a
  planned seed and prepares again. The transport refuses, before transport
  intent, a request whose `dispatch_plan` differs from the reserved plan, and
  refuses an observation whose echoed `dispatch_plan` differs.
- The fixture's `bind-execution` reads a prepared
  `lattice.implementation-attempt-run.v3` request with `worker_compute`,
  `execution_binding`, `network:false` and no namespace, binds a
  `workspace.network=false` plan with its native request profile, and emits a
  planned seed to prepare again. The runbook describes v3, transport v2 and
  the brokered model route.
- `config/implementation-worker.ncl` admits exactly
  `kiln.lattice-implementation-cli.v2`, with positive and negative fixtures.
- The `run` launch passes exactly one environment variable: the one named by
  the admitted request's `omp.model_broker.credential` (`provider:"env"`,
  `key`, no `field`). A missing variable, a malformed reference or key, a
  non-`env` provider or a reserved process-control name is a typed refusal
  before transport intent. No credential keeps today's empty environment.
  `recover` and `cancel` keep the empty environment.
- The transport parses `lattice.implementation-attempt-refusal.v1` bound to
  the request's work, attempt and fence. `dispatched:false` with a reason that
  Lattice's table admits as not dispatched settles the attempt as failed
  before any effect and clears its transport intent; every other refusal is
  retained for reconciliation. `not_started_closed_by_recovery` is settled the
  same way as no execution and is never passed to `reconcile_dispatch`.
- `kiln_core::implementation::AttemptLedger` gains the pure transition
  `settle_dispatch_without_execution`.

## Impact

- **Files**: `crates/kiln-core/src/implementation/attempt.rs`,
  `crates/kiln-implementer/src/lib.rs`,
  `crates/kiln-adapter-lattice/src/worker.rs` and a new
  `worker_settlement.rs`, `crates/kiln-web-service/src/implementation_campaign/binding.rs`,
  `crates/kiln-web-service/src/bin/implementation-fixture.rs`,
  `config/implementation-worker.ncl` with fixtures,
  `docs/implementation-runbook.md`, `CHANGELOG.md`.
- **Compatibility**: Breaking for prepared campaign requests without
  `dispatch_plan`; they are refused before transport and must be prepared
  again. Transport v1 profiles no longer pass the Nickel contract.
- **Testing**: fmt, clippy `-D warnings`, workspace tests, the Nickel fixtures
  script, Cairn validate/traceability/change gates, Octet, and a composed local
  requalification against the real Lattice `db64cd65` binary with pinned OMP
  and a local HTTP 400 endpoint.
