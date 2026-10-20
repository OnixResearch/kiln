# HTTP observation shell verification

Date: 2026-10-20

## Scope

Added `kiln-adapter-http-observe`, a bounded HTTPS shell over the accepted
`kiln-observe-core` projection. The shell serves canonical JSON views, a finite
bounded SSE event stream, and strict request admission. It adds no view
vocabulary, projection logic, control intake, or canonical state.

## Evaluation outcome

- `host-control-core` was evaluated and is **not adopted** for this change. It
  has host-topology and host-event DTOs but no HTTP request, response, or SSE
  frame contract. No host-control source is imported.
- Basalt/UCAN was evaluated and is **not imported** in this source candidate.
  The repository has no pinned basalt/UCAN cohort; basalt adds bounded-http,
  replay-ledger, revocation, secretspec, and an AGPL UCAN/verified-logic
  snapshot. The versioned `ObservationAuth` port remains the binding point. The
  exact source revision, license map, third-party notices, and runtime cohort
  must be recorded before a future import.

The accepted HTTP observation spec is policy-neutral about the auth backend.
The reference verifier is `ed25519_request_v1`, which is fully implemented and
tested.

## Positive coverage

- Complete HTTP observation profile admission, including the embedded accepted
  projection profile and required non-claims.
- Registered origin and `Host` admission.
- State summary, run list, run detail, attempt detail, event tail, and log range
  served as compact canonical JSON with the accepted `kiln.observe.view.v1`
  identity.
- `GET /v1/live` delivers a newly committed event as a bounded SSE `observe`
  frame.
- Authorized requests are admitted before projection.
- TLS 1.2+ serving with a registered certificate chain and key.
- Bounded request read, header, query, connection, timeout, view, error, SSE
  frame, event, and duration limits.

## Negative coverage

- Profile drift: non-HTTPS origin, zero bound, unsupported auth kind, missing
  required non-claim, relative TLS path, and unsupported content contract fail
  admission.
- Unknown view kinds and unknown run/attempt/log-stream targets return bounded
  `404` rejections without projection.
- Missing, stale, replayed, wrong-key, and wrong-target authorization returns a
  bounded `401` rejection.
- Malformed request lines, duplicate or unknown query fields, invalid values,
  non-GET methods, and origin mismatches return bounded `4xx` rejections.
- Oversized requests and header counts fail at their named bounds.
- Request read timeout maps to `408`.
- A plain-TCP TLS failure closes only that connection; a subsequent valid TLS
  request is served.
- Live buffer overflow drops the oldest event, reports `lag` with the cumulative
  dropped count, and never mutates canonical state.
- A conformance manifest missing a required negative case fails admission.

## Commands and results

- `cargo test -p kiln-adapter-http-observe`: PASS, 14 tests.
- `cargo clippy -p kiln-adapter-http-observe --all-targets -- -D warnings`: PASS.
- `cargo fmt --check`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.nickel --no-link -L`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.clippy --no-link -L`: PASS.
- `nix build path:$PWD#packages.x86_64-linux.kiln --no-link -L`: PASS;
  workspace tests include the HTTP observation tests.
- `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json`:
  PASS.
- `cairn traceability coverage --root . --policy
  cairn-policy/generated/cairn-policy.json`: PASS, 731/731 referenced.
- `cairn determinism coverage-audit --root .`: PASS.
- `cairn agent-receipt smoke-mcp --root .`: PASS with the active-change policy
  hash bound in the maintained fixture.
- `cairn release-readiness --root .`: PASS.

## Test TLS material

`fixtures/http-observation/tls/` contains a test-only CA, server chain, and
private key for the loopback integration test. They are committed test fixtures,
not deployment credentials. The profile template references operator-owned
`/var/lib/kiln/observe/tls/` paths.

## Blocked task

`Import and bind a pinned basalt/UCAN cohort behind the observation-authn port`
remains open. It requires an exact pinned source cohort, license map,
third-party notices, and runtime review. This source candidate does not claim
that binding and does not archive the change.

## Non-claims

Passing HTTP observation checks do not prove transport acceptance, live
completeness, control execution, provider correctness, CI success, or release
readiness. They prove only the checked profile, request, projection, bounds,
authorization decision, and stream observation.
