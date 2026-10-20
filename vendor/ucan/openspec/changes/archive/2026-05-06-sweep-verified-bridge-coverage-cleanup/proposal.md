## Why

The current UCAN audit found that the boundary documents can drift from the actual public verified bridge exports and live verifier routing. The recent hygiene pass fixed known stale text, but future verified-logic drains need one source-of-truth sweep that classifies every verified bridge as live-path routed, public helper only, intentionally unrouted, or unsupported by executable vendored runtime code.

## What Changes

- Add an explicit bridge coverage inventory for every public `src/lib.rs` verified export and every vendored `verified_logic::ucan_*` module relevant to UCAN.
- Clean up docs, canonical specs, and bridge guards so public-helper-only bridges cannot be mistaken for live verifier guarantees.
- Route any implementation-ready exact vendored bridge into live paths only when the live shell already supplies the same finite data-only facts.
- Preserve explicit trust boundaries for parser, codec, crypto, resolver/proof-store/revocation backend, clock, provenance, and caller policy behavior.

## Scope

In scope:

- `src/lib.rs`, `src/verified.rs`, live verification paths in `src/token.rs`, bridge guard scripts, `docs/verified-boundaries.md`, and canonical OpenSpec library text.
- A one-pass inventory and cleanup of bridge exposure/routing claims.

Out of scope:

- New broad verified-logic primitives.
- Claims that CID, principal-link, cryptographic, parser, backend, clock, or semantic identity correctness are formally verified unless an exact executable vendored bridge exists and is routed.
- Network, filesystem, process-global resolver, default proof-store, revocation registry, replay store, persistence, async runtime, or ambient-clock behavior.

## Verification

- `openspec validate sweep-verified-bridge-coverage-cleanup --strict`
- `openspec validate --all --strict`
- focused Rust tests for any changed bridge/live path
- `./scripts/check-ucan-bridge.sh`
- `git diff --check`
- full UCAN gates if implementation changes beyond docs/specs are made
