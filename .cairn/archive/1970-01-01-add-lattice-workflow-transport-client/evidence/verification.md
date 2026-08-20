# Verification evidence

Date: 2026-08-20

## Producer identity

Kiln pins `lattice-workflow-contract` and `lattice-workflow-transport` at revision `e3148ec59fa12081c270d9800a060d94214b9405`.

The fixture synchronization check passed against a detached checkout of that exact revision. Contract, transport, and dispatch-recovery fixtures match the producer.

## Rust evidence

- workspace tests: pass
- `kiln-adapter-lattice`: 17 tests pass
- strict workspace Clippy with `-D warnings`: pass
- `kiln-core` on `wasm32-unknown-unknown`: pass
- format check: pass

The concrete tests cover dispatch, observe, pause, resume, log-read, reconciliation, cancel, unavailable endpoint, recovered acceptance, and not-found recovery.

## Nickel evidence

The Lattice workflow profile and positive fixture export successfully.

Negative fixtures reject capability drift, zero bounds, empty peer sets, unsupported controls, unsupported effects, and unsupported nodes.

## Nix evidence

`nix flake check path:$PWD -L` passed all checks.

## Lifecycle evidence

Cairn proposal, design, and task gates passed. Cairn validation passed before and after spec sync.

Tracey passed with `401/401` referenced requirements. The traceability receipt BLAKE3 was `4b746e7260375fd279b158735419917cf3d5cba2f00c5a125f1998d5a2ea4e89`.

## Claim boundary

This evidence proves the checked local client, exact producer pin, bounded mapping, and tested uncertainty behavior.

It does not prove a deployed Lattice handler, durable lookup, provider availability, workflow correctness, external effect success, or Radicle CI completion.
