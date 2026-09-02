# Proposal: Repair Celld Worker traceability with executable evidence

## Why

The accepted `celld-event-store-adapter` specification has 22 scenario identities without source evidence, while three source markers (`kiln.celld.worker`, `.error_ownership`, `.composition_root`) have no accepted requirement. The archived change recorded model-only Worker fixtures even though some accepted scenarios describe actual Durable Object SQLite transactions, process restarts, and observed runtime cohorts. Neither an in-memory clone nor the client's empty `load` result can verify those claims.

## What Changes

- Repair dangling marker ownership and attach scenario evidence only to meaningful positive and negative behavior, not to comments, mocked responses, or unrelated success paths. r[kiln.celld.local_worker_evidence]
- Supply the Kiln-owned Worker artifact referenced by `config/celld-wrangler.ncl`, with authenticated bounded requests and a real SQLite-backed local conformance path where the pinned cohort API permits it. Preserve provider-neutral core and do not conflate a local SQLite process with a celld fleet. r[kiln.celld.local_worker_evidence]
- Correct the Rust shell's false empty-read result and response-loss classification. Keep ambiguous mutations unknown until exact operation reconciliation, with no unsafe implicit retry. r[kiln.celld.local_worker_evidence]
- Probe pinned source, executable, bundler, local storage/runtime options, and distinguish actually executed Worker paths from unavailable live-fleet claims in evidence. Retain all accepted obligations; do not remove, relabel, or invent verification merely to turn global traceability green. r[kiln.celld.local_worker_evidence]
- Build the pinned Celld executable from its hash-checked source in a self-cleaning
  local SeaweedFS S3 cohort, run the real Worker through gated Celld and scoped
  HTTPS, and exercise `EventStorePort` append/load/reconciliation from Rust.
  Conditional lease and schema-migration routes must preserve SQLite facts
  across local node restart and reject mixed or disabled-gate declarations
  before a new canonical append. r[kiln.celld.local_worker_evidence]
- Bind local admission to fresh trusted-operator measurements of executable,
  Worker deployment, storage, ingress, output gate and probe, then cross-check
  the served Worker version over strict TLS. Contradictions must fail closed;
  no Celld-signed remote attestation is implied. r[kiln.celld.local_worker_evidence]

## Scope and Non-Claims

Kiln's Celld Worker and Rust adapter, native tests, source/policy evidence, and existing status adoption lifecycle package are in scope. Unrelated active evaluator-cohort changes remain untouched. No hosted Radicle operation, hosted fleet mutation, private operator API use from normal event-store traffic, or credentials are authorized. A locally executed Worker/SQLite test alone is not proof of celld's output gate, conditional-write probe, node takeover, or distributed durability. The separate pinned local cohort runner can prove only the process settings, startup probe, and authenticated reads it actually observes, never another fleet's remote durability or hostile isolation. Sync/archive are conditional on actual global traceability passing under the existing policy.

## Affected Specs

- `celld-event-store-adapter`: add a local-worker evidence discipline. Its previously accepted production obligations remain unchanged.
