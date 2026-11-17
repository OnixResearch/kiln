## Context

`crates/kiln-web-service/tests/http_contract.rs` validates real `Engine::api`
responses against `contracts/http-api/openapi.json` with an independent
schema-subset validator, compares error goldens and classifies bodies that were
already collected in memory (`classify`). `crates/kiln-web-service/tests/engine/runtime.rs`
already starts the real `serve` process over loopback TLS with the fixture
certificates in `fixtures/http-observation/tls/`, mode-0400 token files, an
ephemeral port and a readiness wait. `docs/http-api-contract.md` records the
lockstep decision, the client classes and a prose Drift/Progenitor no-go.

## Goals and Success Contract

A reviewer can see, from retained tests, a CI check and evidence, that:

- the lockstep client, talking HTTPS to the real service process, classifies
  every documented success and error response, and follows pagination to its
  end or reports it incomplete;
- network faults (oversized, truncated, mislabeled, slow or stalled bodies)
  become `transport` outcomes and never authentication, admission or schema
  outcomes, and no request is retried;
- a contract change that Drift calls incompatible or unhandled fails CI unless
  it is an intentional version transition;
- the code-generation decision cites retained measurements.

## Decisions

### Decision: exercise the served shell, not only the engine

**Choice:** a test starts the release-equivalent `kiln-web-service serve`
binary with the runtime fixture pattern and sends every request through the
bounded client. A wrong bearer token yields the server's real `401`; worker and
viewer scopes yield `403 worker_scope` and `403 execution_binding_scope`; an
unknown run yields `404 run_missing`; an out-of-range limit yields
`400 page_bound`. Successful bodies validate against the committed document.
Pagination follows `next_cursor` until it is absent; a follow-up that the
server refuses (`cursor_invalid`) or that fails in transport marks the listing
incomplete rather than complete.

**Rationale:** the issue asks for the matrix against the real bounded service
shell. Authentication, TLS and framing are owned by `server.rs`, which the
in-process engine tests never reach.

### Decision: mutated bodies come from a loopback TLS stub

**Choice:** the real service never sends malformed bodies, so a test-only TLS
stub on loopback, using the same fixture certificate, replays real response
bodies captured from the served shell with one mutation each: a missing
required field, a wrong type, an extra field where `additionalProperties` is
false, truncated JSON, a `Content-Length` larger or smaller than the body, an
oversized chunked body, a body that drips below the idle deadline until the
total deadline, and a body that stalls.

**Rationale:** classification must hold over a real socket; deriving expected
results from the same Rust types on both sides is excluded by the issue.

### Decision: bounded reads with named deadlines and no retries

**Choice:** the client reads the body incrementally up to a named byte limit
for every status, independently of `Content-Length`, and enforces named
connect, total and body-idle deadlines. Exceeding a limit, a deadline,
truncation or a connection failure is `transport`. Statuses map as today:
`401` authentication, `403` authorization, `404` not_found, a `2xx` body that
fails validation `schema_mismatch`, any other status `unknown`. One request is
one attempt.

**Rationale:** the issue states that `Content-Length` alone is not a cap and
that the default is no retries.

### Decision: Drift runs as a pinned CI tool with a reviewed baseline

**Choice:** `tools/http-contract-drift` is a small binary with its own lockfile
that depends on Drift at `dad4f398c10ca53298d2fec0586d834c8e85daf1`. The
reviewed baseline is a committed copy of the last accepted document. The policy:

- incompatible (in either direction): fail, unless the current document's
  contract version differs from the baseline's, which is the intentional
  version transition and must also replace the baseline;
- unhandled: fail; a reviewer resolves it by inspecting the change and
  replacing the baseline in the same commit;
- trivial or compatible: pass and print every change.

A flake check runs the tool on the baseline and the committed document, and on
fixture pairs that must produce each outcome. Drift's and its transitive
licenses are recorded for this tool-only use.

**Rationale:** the issue asks for Drift in CI with explicit policy for each
classification and fail-closed handling of unhandled comparisons. A reviewed
baseline gives a comparison even under the lockstep decision.

### Decision: measure generation in a scratch workspace

**Choice:** generate a Progenitor client at
`b317a86578098ecabbeb04a0f57dc693032b81cc` for the committed document outside
the Kiln workspace, record generated lines and bytes, added dependency count
and licenses, and cold and warm `cargo build` wall times on this host, next to
the handwritten client fixture's size. The adoption decision in
`docs/http-api-contract.md` cites those numbers.

**Rationale:** the issue accepts a documented no-go only on the basis of a
comparison; the first pilot did not retain one.

## Risks / Trade-offs

- Drift may report OpenAPI 3.0.x constructs as unhandled. Failing closed means
  such changes need a reviewed baseline update; that is the requested behavior.
- Wall times on a shared host vary. They are single-host observations, not
  benchmarks.
- The stub replays captured bodies; it is a client-classification fixture, not
  server conformance evidence.

## Verification Strategy

Focused `cargo test -p kiln-web-service` for the contract tests, the new flake
check with its fixture outcomes, Clippy and formatting, and the Cairn proposal,
design and tasks gates. Evidence records each command with its result and the
measurements.

## Non-claims

No server migration, no compatibility promise beyond the documented lockstep
window, no adoption of a generated client, and no production or hostile-network
qualification.
