# Iroh observation shell verification

Date: 2026-10-20

## Scope

Added `kiln-adapter-iroh-observe`, a bounded `kiln.observe.v1` iroh shell over
the accepted `kiln-observe-core` projection. The shell serves accepted views to
allowlisted peers, carries BLAKE3 payload identities for log bytes and event
records, denies unknown or revoked peers, and proves HTTP/iroh logical-view
equality over shared fixtures. It adds no view vocabulary, projection logic,
control intake, or canonical state.

## Evaluation outcome

- Consumed cohort: `iroh` `0.95.1`, ALPN `kiln.observe.v1`.
- `iroh-blobs` and `bao-tree` are deferred. The bounded one-stream
  request/response frame carries Kiln-owned BLAKE3 payload identities, and the
  recipient verifies them before claiming delivery. A future import must record
  the exact verified-streaming cohort and review.
- `molten` (aspen) is not adopted as the transport home for this change. No
  pinned molten runtime cohort is present in this source candidate, and its
  `iroh_sam_dataspace` retains unestablished tracey debt. The pinned iroh cohort
  remains behind the transport port.

## Positive coverage

- Complete profile admission for the pinned cohort, ALPN, endpoint, relay,
  peer, frame, payload, timeout, and non-claim facts.
- Trusted peer serves a state-summary view over a local iroh endpoint pair.
- Log-range serving carries `log_bytes` BLAKE3 identity and the recipient
  verifies it.
- Event-tail serving carries `event_records` BLAKE3 identity and the recipient
  verifies it.
- HTTP and iroh legs produce equal logical views and equal canonical JSON bytes
  over shared fixtures.
- Stable `did:key` Ed25519 derivation from an endpoint identity.
- Bounded frame, payload, connection, and deadline admission.

## Negative coverage

- Cohort drift, unsupported ALPN, zero frame bound, missing required non-claim,
  and non-HTTPS relay fail profile admission.
- Unknown and revoked peers are denied before a view is served.
- Corrupted payload bytes with a stale identity are rejected.
- Transport failure and a closed endpoint return bounded errors and never
  return a `view`.
- Divergent HTTP/iroh logical views fail equivalence admission.
- A conformance manifest missing a required negative case fails admission.
- Relay URLs are explicit and visible; an HTTP relay is rejected. No relay fact
  enters the logical view.

## Commands and results

- `cargo test -p kiln-adapter-iroh-observe`: PASS, 12 tests.
- `cargo clippy -p kiln-adapter-iroh-observe --all-targets -- -D warnings`: PASS.
- `cargo fmt --check`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.nickel --no-link -L`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.clippy --no-link -L`: PASS.
- `nix build path:$PWD#packages.x86_64-linux.kiln --no-link -L`: PASS; the
  workspace test run includes the iroh observation tests.
- `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json`:
  PASS.
- `cairn traceability coverage --root . --policy
  cairn-policy/generated/cairn-policy.json`: PASS, 750/750 referenced.
- `cairn determinism coverage-audit --root .`: PASS.
- `cairn agent-receipt smoke-mcp --root .`: PASS with the active-change policy
  hash bound in the maintained fixture.
- `cairn release-readiness --root .`: PASS.
- `cairn sync observation-iroh-shell --root . --execute`: PASS; merged the
  delta into `.cairn/specs/observation-iroh-shell/spec.md`.
- `cairn archive observation-iroh-shell --root . --execute`: PASS; the change is
  retained at `.cairn/archive/1970-01-01-observation-iroh-shell`.
- Post-archive `cairn validate`, traceability coverage (`750/750`), and
  `cairn release-readiness`: PASS. The change-specific requirement source was
  removed from the policy because the accepted spec now lives under
  `.cairn/specs`.

## Non-claims

Passing iroh observation checks do not prove transport acceptance, live
completeness, verified delivery of unverified payloads, control execution,
provider correctness, CI success, or release readiness. They prove only the
checked profile, protocol, peer, payload, bounds, and view identity.
