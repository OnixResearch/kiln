# Verification evidence

## Result

The standalone Gitea/Forgejo adapter is implemented and exercised. It accepts
the documented Gitea, Forgejo and GitHub-compatible webhook header aliases,
authenticates the exact original body, admits push and pull-request revisions,
persists durable replay, dispatches the reviewed Lattice workflow, and projects
bounded commit statuses through the Gitea API. `kiln-gitea-job` acquires only
the admitted exact commit and runs reviewed argv.

## Evidence

- `cargo test -p kiln-adapter-gitea --lib`: 54 passed, covering profile and
  origin admission, raw/prefixed signatures, Gitea `synchronized` and
  `pull_request_sync` events, fork denial, replay, status writes/reconciliation,
  DNS pinning and exact-source job admission.
- `cargo test -p kiln-adapter-gitea --features protocol-fixtures --test
  local_ci_e2e --test job`: 22 passed, covering exact-source success and
  failure, duplicate delivery, restart recovery without redispatch, invalid
  signature and framing, fork denial, ambiguous status reconciliation and
  conflicting terminal evidence.
- `cargo test --workspace`, `cargo check --workspace --all-targets` and
  `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo fmt -- --check` passes. The change includes one pre-existing
  formatting-only normalization in the Radicle canonical-tag test.
- Nickel typechecks and conformance exports pass for all three Gitea profiles.
- The sandboxed `gitea-protocol-fixtures` Nix check passes with the full local
  E2E rail.
- `cairn validate --root .` reports zero issues.

## Non-claims

Local fixtures do not prove a live Gitea or Forgejo deployment, remote status
mutation, source trust, workflow correctness, host confinement or release
eligibility. The optional `kiln-hosted` connection composition remains
GitHub-only. SHA-256 repositories and form-encoded webhook deliveries are not
supported.
