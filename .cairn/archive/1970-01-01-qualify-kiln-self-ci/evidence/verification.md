# Verification

## Real self-CI

Command (fresh local and remote roots; replace both `<fresh>` placeholders):

```sh
nix develop path:. --no-write-lock-file -c python3 scripts/radicle-ci-live-self-ci.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel /nix/store/5yqkl5bk4d335hnbss23w4a2crfafxlp-nickel-1.17.0/bin/nickel \
  --nixpkgs /nix/store/1dffznc7fgw6xnzssy1h4waxz63i11qa-source \
  --worker /nix/store/36vbiak48882whr5dwfrls2q6f696n6f-kiln-0.1.0/bin/kiln-onixos-worker \
  --extension /nix/store/36vbiak48882whr5dwfrls2q6f696n6f-kiln-0.1.0/bin/kiln-aspen-extension \
  --coordinator /nix/store/36vbiak48882whr5dwfrls2q6f696n6f-kiln-0.1.0/bin/kiln-radicle-ci \
  --host /nix/store/y6fp5140x5d9n3g1k7dl13pr9riyim06-kiln-aspen-host-0.1.0/bin/kiln-aspen-host \
  --radicle /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/rad \
  --radicle-node /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/radicle-node \
  --broker /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cib \
  --broker-tool /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cibtool \
  --jj /nix/store/hwifmhfi50y1595lhwiqsn7acppa7zdm-jujutsu-0.45.1/bin/jj \
  --git /nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0/bin/git \
  --secretspec /nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec \
  --root /var/tmp/kiln-self-ci-<fresh> --remote-host aspen1 \
  --remote-root /var/tmp/kiln-self-ci-<fresh> \
  --remote-address 100.100.103.95 --local-address 100.110.43.11 \
  --remote-port 52000 --remote-readiness-seconds 60 \
  --base-revision 305f85d80b9ea05fbd91db73de8a6662a55650ee
```

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
