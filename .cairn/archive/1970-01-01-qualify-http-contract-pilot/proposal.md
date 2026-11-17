# Why

Issue #73's first pilot, archived in
`.cairn/archive/1970-01-01-pilot-http-api-contract/`, validates real
`Engine::api` responses in process against `contracts/http-api/openapi.json`.
Four acceptance items remain open:

- the old/new matrix must run against the real bounded service shell, not only
  the in-process engine;
- the client must bound received success and error bytes before parsing and
  enforce total and body deadlines (`docs/http-api-contract.md` says network
  deadlines are not exercised);
- Drift must run in CI with an explicit policy for each classification;
- build time and generated-code volume must be measured and retained, not only
  argued.

## What Changes

- Run the lockstep client fixture against the actual HTTPS
  `kiln-web-service serve` process on loopback: success bodies, 400, 401, 403
  and 404 error variants, and cursor pagination to the last page. Keep
  authentication, admission, transport and schema failures distinct.
- Run the same client against a loopback TLS stub that returns mutated real
  bodies: missing required, wrong-type and extra fields, truncated JSON, a
  `Content-Length` that disagrees with the body, an oversized chunked body, and
  slow and stalled bodies. The client bounds received success and error bytes
  independently of `Content-Length`, enforces connect, total and body-idle
  deadlines and never retries.
- Add a pinned Drift comparison tool under `tools/` and a flake check that
  compares the committed document with a reviewed baseline. Incompatible
  changes fail unless the contract version changes; unhandled comparisons fail;
  trivial changes pass and are printed. Fixture pairs prove each outcome.
- Generate a Progenitor client for the committed document in a scratch
  workspace, measure generated-code volume, dependency count and cold and warm
  build time against the handwritten client fixture, and retain the
  measurements with the adoption decision.

## Impact

Tests, a CI tool, a flake check, documentation and evidence. No endpoint,
server behavior or workspace dependency changes. Drift is a pinned CI tool, not
a shipped dependency; Progenitor is measured, not adopted.

## Authority

The operator asked to fix the open issues whose remaining work is isolated to
Kiln. Publication of the result follows the operator's standing request to
commit and push; no other remote mutation.
