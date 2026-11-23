# UCAN Dependency Posture Audit

This file records the checked dependency-advisory posture of the locked UCAN
workspace. It does not rewrite the policy in `deny.toml`; that file
is the canonical policy. The posture gate in
`tools/check-dependency-posture.rs` and the `dependency-posture` Nix check
enforce the recorded result.

## Assessment

- Assessment date: 2026-11-22
- Assessment basis: the committed `Cargo.lock` at the change revision
  integrated as `move-nostr-adapter-to-k256-0-14`. The original audit
  (2026-08-16, `track-dependency-posture`) set the policy below.
- Advisory database: RustSec `advisory-db` revision
  `55e1c3dfbd834237a05bbbf7b50ba3b5be27b662`.
- Result: zero exploitable vulnerabilities in the locked dependency set at
  the assessment revision. `cargo audit` reports no vulnerability. Its two
  warnings are `RUSTSEC-2024-0436` and `RUSTSEC-2026-0486`, listed below.

## Resolved advisory

- `RUSTSEC-2026-0387` (`crypto-bigint`): timing variability in operations
  intended to be constant-time; patched in `0.7.6`. Before this change,
  `k256 0.13.4` was the only path to `crypto-bigint 0.5.5`, through
  `elliptic-curve 0.13.8`. The Nostr adapter now requires exact
  `k256 0.14.0`, which resolves `crypto-bigint 0.7.11`. The advisory left
  the locked graph; it is not pinned in `deny.toml`.

## Advisory status at the 2026-11-22 audit

These results use advisory-db revision `55e1c3df`:

- `RUSTSEC-2024-0436` (`paste 1.0.15`, unmaintained): applies, and it is
  pinned in `deny.toml`. It is a dev-only dependency reached through
  `hegeltest`; no runtime path uses it.
- `RUSTSEC-2026-0190` (`anyhow` `downcast_mut` unsoundness): not encountered.
  The lock resolves `anyhow 1.0.104`, so `cargo-deny` reports the pin as
  unused. The pin stays in `deny.toml` only because
  `tools/check-dependency-posture.rs` requires it. Its original rationale
  still describes the earlier exposure: the vendored `repo_index` tooling
  module uses `chain()` and never calls `downcast_mut`.
- `RUSTSEC-2026-0486` (`rustix 1.1.4`, unsound xattr buffer handling):
  applies, and it is open. It is not pinned. It is reached only through the
  dev-only `tempfile` dependency of `hegeltest`, `proptest`, and the root
  test suite. `cargo-deny check advisories -D warnings` reports it.

`cargo-deny check advisories -D warnings` treats any unpinned applicable
advisory as an error, so the open `rustix` finding fails that scan until a
reviewed policy change pins or removes it. Dropping the stale `anyhow` pin
also needs a reviewed change to the posture checker. The license allow list
covers the workspace licenses.

## Enforcement

- `tools/check-dependency-posture.rs` validates the policy and the lockfile
  deterministically: both pins must carry a non-empty rationale, the
  license section must deny unlicensed packages, the lockfile must still bind
  `paste` and `anyhow`, the lockfile must resolve exactly `k256 0.14.0` and
  no `crypto-bigint` below `0.7.6`, and the positive fixture must pass while
  the negative fixture must fail.
- The Nix `dependency-posture` check runs the checker and then invokes
  `cargo-deny check advisories` against the committed `Cargo.lock` with the
  committed policy. The check never resolves a fresh dependency set.
- `tests/dependency_posture.rs` asserts the same markers under `cargo test`
  without adding runtime crate code.

## Non-claims

This posture record covers the locked dependency set only. It does not claim
the absence of vulnerabilities in other revisions, upstream crates that the
workspace does not link, or future dependency bumps. A future bump must re-run
the audit before the posture record is updated. The record does not claim
cryptographic, authority, or runtime correctness.
