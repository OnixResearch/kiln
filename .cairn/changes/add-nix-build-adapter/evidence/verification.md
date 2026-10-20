# Verification: Exact Nix Build Provider

Date: 2026-10-19
Workspace: `/home/brittonr/git/OnixResearch/kiln`

## Pinned cohort

- nixpkgs revision: `346dd96ad74dc4457a9db9de4f4f57dab2e5731d`
- Nix version: `2.34.8`
- `nix`: `/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix`
- `nix-store`: `/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix-store`
- executable BLAKE3:
  `82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852`
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525`

## Commands and outcomes

| Command | Outcome |
|---|---|
| `cargo test -p kiln-adapter-nix --offline` | 14 fixture tests passed. |
| `cargo test -p kiln-nix-build --offline` | 3 composition fixture tests passed. |
| `tests/nix-build-live.sh` | 15 adapter tests (14 fixture + 1 live) and 4 composition tests (3 fixture + 1 live) passed. |
| `cargo clippy -p kiln-adapter-nix --all-targets --offline -- -D warnings` | Passed. |
| `cargo clippy -p kiln-nix-build --all-targets --offline -- -D warnings` | Passed. |
| `cargo fmt -p kiln-adapter-nix -p kiln-nix-build --check` | Passed. |
| `cargo test -p kiln-conformance --offline` | Passed; `kiln-nix-build` and `kiln-adapter-nix` are rejected from core dependencies. |
| `nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-fixtures -L --no-write-lock-file` | Passed; result `/nix/store/qjnhb0f1dgrycq01j3qch16f64ai6y0x-kiln-nix-build-adapter-fixtures-0.1.0`; 14 adapter and 3 composition fixture tests. |
| `nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-cohort -L --no-write-lock-file` | Passed; result `/nix/store/f3skndbhyizcfdcl2z6dbcvc60n8ppv7-kiln-nix-build-adapter-cohort`; `pkgs.nix` equals the pinned path, version, and BLAKE3. |
| `nix run nixpkgs#nickel -- typecheck config/nix-build-profile.ncl` | Passed. |
| `nix run nixpkgs#nickel -- format --check config/nix-build-profile.ncl config/fixtures/nix-build-positive.ncl` | Passed. |
| `nix run nixpkgs#nickel -- export config/nix-build-profile.ncl` | Passed. |
| `nix build path:$PWD#checks.x86_64-linux.nickel -L --no-write-lock-file` | Passed; result `/nix/store/j13skcng1m8adv4m3wsz7h2m6c56667l-kiln-nickel`. |
| `cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"` | Passed with no issues. |
| `cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json` | Passed with no dangling or missing markers. |
| CLI `from-task --profile … --task … --intent … --log … --release` | `Success`; exact `out` product; bounded log written; product content `cli-final`; effect GC-root directory removed. |
| CLI `doctor --profile …` with a fully bound profile | `{\"profile\":\"nix-build-v1\",\"status\":\"admitted\"}`. |
| CLI `build --profile … --plan … --log … --release` | `Success`; exact `out` product; bounded log written; product content `cli-build`; effect GC-root directory removed. |

## Observed behavior

- Profile admission requires the repo-pinned `nix` and `nix-store` identities,
  nixpkgs revision, Nix version, exact expected output paths, limits, and
  non-claims. Drift fails before dispatch.
- The adapter runs `nix-store --add-root <effect>/root --indirect --realise
  <drv>` with a cleared environment, fixed `HOME`/`NIX_CONFIG`, no builders or
  substituters, bounded output, and process-group teardown.
- One root per expected output is admitted; every target must equal the exact
  expected store path. Missing, malformed, duplicate, or mismatched roots are
  `ProviderError`; nonzero exit is `Failed`; timeout and cancellation are
  terminal; ambiguous process start is `Unknown` without a terminal receipt.
- Successful products remain rooted until explicit idempotent release. A
  discovery-shaped `TaskProposal` maps to the exact profile and plan, and the
  live composition test realizes it through the pinned local store.
- Duplicate dispatch returns the recorded observation; changed plans and
  observation-only restore conflicts fail closed. Log reads are identity-bound,
  bounded, and unavailable after observation-only restore.

## Non-claims

This evidence proves only the exact pinned executables, exact derivation,
local store inputs, exact expected output paths, native realisation roots,
product/receipt/log identities, composition mapping, and reconciliation
behavior exercised above. It does not prove build correctness, sandbox
completeness, derivation provenance or source trust, output content semantics,
cache truth, reproducibility, or release readiness.
