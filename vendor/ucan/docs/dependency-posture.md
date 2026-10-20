# UCAN Dependency Posture Audit

This file records the checked dependency-advisory posture of the locked UCAN
workspace. It does not rewrite the policy in `deny.toml`; that file
is the canonical policy. The posture gate in
`tools/check-dependency-posture.rs` and the `dependency-posture` Nix check
enforce the recorded result.

## Assessment

- Assessment date: 2026-08-16
- Assessment basis: the committed `Cargo.lock` at the change revision
  integrated as `track-dependency-posture`.
- Result: zero exploitable vulnerabilities in the locked dependency set at
  the assessment revision.

## Applicable advisories

The audit found two applicable advisories. Both are pinned with an explicit
non-exploitable rationale in `deny.toml`:

- `RUSTSEC-2024-0436` (`paste`): the crate is unmaintained, but it is a
  dev-only dependency reached through `hegeltest`. No runtime path uses it.
- `RUSTSEC-2026-0190` (`anyhow`): the `downcast_mut` unsoundness reaches only
  the vendored `repo_index` tooling module. That module uses `chain()` and
  never calls `downcast_mut`.

The policy pins both applicable advisories with an explicit rationale.
`cargo-deny check advisories -D warnings` treats any unpinned applicable
advisory as an error. The license allow list covers the workspace licenses.

## Enforcement

- `tools/check-dependency-posture.rs` validates the policy and the lockfile
  deterministically: both pins must carry a non-empty rationale, the
  license section must deny unlicensed packages, the lockfile must still bind
  `paste` and `anyhow`, and the positive fixture must pass while the negative
  fixture must fail.
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
