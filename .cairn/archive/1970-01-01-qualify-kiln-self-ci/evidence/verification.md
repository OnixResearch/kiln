# Verification

## Real self-CI

Command: `nix develop path:. -c python3 scripts/radicle-ci-live-self-ci.py ... --base-revision 305f85d80b9ea05fbd91db73de8a6662a55650ee`

Final report: `evidence/success/verification.json` (`kiln.self-ci.v1`). Canonical repository untouched; dedicated fork `rad:z2J2jH8gk8G8pzjuvvAsYc6jjyih`; source placement publisher/observer remote, CI/broker/controller/worker local.

| Case | Revision | Outcome | Evidence |
|---|---|---|---|
| success | `4a2a3e6a80fd5c8c5f42e4a0ace6c333d26d4e8a` | `Success` | `candidates/success/`, `ingress/success/`; rooted `bin/kiln`; host SIGKILL/restart; duplicate fetch preserved the operation |
| failure | `86e35b5e9d191f9992fbc3c5fb85a8c0bf51db16` | `DomainFailure` | `candidates/failure/`, `ingress/failure/`; genuine Nix diagnostic contains `self_ci_genuine_failure`; no rooted output |
| unavailable | `995da1ff126ece7bbca91f5fc014bf100e4818f7` | `Pending` | `candidates/unavailable/`, `ingress/unavailable/`; no status intent and no finished broker reply |

Rejected and partial attempts remain under `evidence/rejected-attempt-*` and `evidence/partial-attempt-n/`: alias rejection, stock 60 s build deadline, stale coordinator cohort, host disconnect termination and truncated source reads.

## Focused checks

- Pure alias mapping: `cargo test -p kiln-core aliased_derivations_are_admitted` passed.
- Host disconnect regression: the `kiln-aspen-host` Nix package check phase ran the `hosted` suite (13 passed), including the abandoned-client exchange.
- Python harness: `python3 -m py_compile scripts/radicle-ci-live*.py` passed.
- Typed policy: `nickel typecheck config/radicle-ci-live-self-ci.ncl` and `config/onixos-ci-pilot-self-ci.ncl` passed.

## Required gates

| Gate | Result |
|---|---|
| `cargo test --locked --workspace` | passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | passed |
| `cargo fmt --check` (workspace and host manifest) | passed |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | valid |
| `nix build path:$PWD#checks.x86_64-linux.octet --no-link -L` | 0 warnings, 0 errors |
| `nix flake check path:$PWD --no-write-lock-file -L` | all checks passed; host suite 13/13 |
