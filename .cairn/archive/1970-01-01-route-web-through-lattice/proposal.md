## Why

The direct kiln-web-hosted composition bypasses Lattice and creates a second workflow orchestration path. The operator explicitly requires Web workflow orchestration to remain in Lattice. Keep Kiln approval/source semantics and hosted execution behind their existing boundaries rather than inventing a Lattice-shaped forwarding server.

## What Changes

- All Web projects dispatch through the existing LatticeAdapter and native Lattice workflow-exchange application. Remove the direct provider selector, binary, factory, adapter and typed bridge profiles; no compatibility alias.
- A reviewed native Lattice shell node invokes a fixed-profile kiln-hosted lattice-job helper. Lattice owns DAG scheduling, execution identity, lifecycle, cancellation and recovery; hosted runs the node's reviewed backend pipeline.
- Expose an authenticated read-only exact Web execution-binding projection. Hosted independently fetches it over operator-pinned HTTPS before new execution, verifies the stored approval/run/node/execution identity and independently acquires exact Git bytes. A worker assertion alone is not approval authority.
- Preserve once-only per-Web-run/per-node backend execution, native Lattice outcomes, captured Web stdout/stderr, live cancellation and no-secret untrusted host admission.
- Retain prior direct-route receipts only as explicitly superseded history. Exercise the actual native Lattice runtime in new evidence; no fake executor, production deployment, host waiver or hostile-isolation claim.

## Impact

- **Files**: kiln-web-service profile/server/worker binding and client surfaces; kiln-hosted backend admission, CLI/job client and policies; provider hooks; Nickel fixtures; flake apps; runtime recipe; existing operator/readiness docs and source specs.
- **Testing**: binding/authority and replay negative regressions, existing Lattice contracts, strict compiled checks, typed positive/negative configuration checks, native Cairn gates, and actual HTTPS Web → native Lattice → authenticated hosted execution including trusted stdout/cancellation/restart and exact untrusted kernel refusal.
- **Owner**: Kiln Web/hosted maintainers own the integration and repeatable recipe; Lattice owns the unmodified native workflow runtime. Operators retain source, issuer, credential and host authority.
