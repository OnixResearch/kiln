# Webhook adapter validation

Date: 2026-08-20

## Baseline

- `nix develop path:$PWD -c cargo test --workspace` passed before implementation.

## Focused checks

- `cargo test -p kiln-adapter-webhook` passed with eight positive and negative tests.
- Focused Clippy passed with `-D warnings`.
- `nickel export config/webhook-profile.ncl` passed.
- Dynamic-destination, inline-secret, and zero-bound Nickel fixtures failed as required.

## Repository checks

- Request projection binds the run, outcome, policy, endpoint, BLAKE3 body identity, and idempotency key.
- Private addresses stop before secret access.
- Redirects, TLS errors, oversized responses, rate limits, and ambiguous timeouts have explicit outcomes.
- BLAKE3 signatures are deterministic, and diagnostics redact secret material.
- Workspace tests, Clippy, `no_std`, and all Nickel fixtures passed.
- Cairn validation passed after spec sync.
- Tracey coverage passed at `332/332` referenced requirements.
- `nix flake check path:$PWD -L` passed all checks.

## Reuse decision

`bounded-http` revision `db13eb8a0341719e54817ae34451cf310a41033f` was evaluated. It was not imported because it requires Rust `1.96.0`; Kiln is pinned to `1.88`.

## Non-claims

These checks prove bounded request construction and observed delivery classification. They do not prove human receipt, endpoint trust, message effect, or CI success.
