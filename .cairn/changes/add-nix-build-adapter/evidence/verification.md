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
- Nix policy: sandbox enabled; substituters, remote builders, and experimental
  features disabled through `NIX_CONFIG`; `NIX_CONF_DIR` fixed to the admitted
  home.

## Commands and outcomes

| Command | Outcome |
|---|---|
| `cargo test -p kiln-adapter-nix --offline` | 14 fixture tests passed. |
| `cargo test -p kiln-nix-build --offline` | 5 composition fixture tests passed, including preflight failure and durable-state round trip. |
| `tests/nix-build-live.sh` | 18 adapter tests (14 fixture + real success, timeout, cancellation, multi-output), 6 composition tests (5 fixture + live task build), and 1 real discovery-to-build test passed. |
| `cargo clippy -p kiln-adapter-nix --all-targets --offline -- -D warnings` | Passed. |
| `cargo clippy -p kiln-nix-build --all-targets --offline -- -D warnings` | Passed. |
| `cargo fmt -p kiln-adapter-nix -p kiln-nix-build -p kiln-adapter-nix-eval --check` | Passed. |
| `cargo test -p kiln-conformance --offline` | Passed; `kiln-nix-build` and `kiln-adapter-nix` are rejected from core dependencies. |
| `nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-fixtures -L --no-write-lock-file` | Passed; result `/nix/store/s1a4pvf3a97p63vpxnzqcdynzq567g60-kiln-nix-build-adapter-fixtures-0.1.0`; 14 adapter and 5 composition fixture tests. |
| `nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-cohort -L --no-write-lock-file` | Passed; result `/nix/store/f3skndbhyizcfdcl2z6dbcvc60n8ppv7-kiln-nix-build-adapter-cohort`; `pkgs.nix` equals the pinned path, version, and BLAKE3. |
| `nix run nixpkgs#nickel -- typecheck config/nix-build-profile.ncl` | Passed. |
| `nix run nixpkgs#nickel -- format --check config/nix-build-profile.ncl config/fixtures/nix-build-positive.ncl` | Passed. |
| `nix run nixpkgs#nickel -- export config/nix-build-profile.ncl` | Passed. |
| `nix build path:$PWD#checks.x86_64-linux.nickel -L --no-write-lock-file` | Passed; result `/nix/store/r52aw90l678lbhyzmbllygad3fvl42k7-kiln-nickel`. |
| `cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"` | Passed with no issues. |
| `cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json` | Passed with no dangling or missing markers. |
| CLI task state: `from-task --state …`, `observe --state …`, `release --state …`, repeat `release`, repeat `from-task` | `Success`; observe matched; release removed roots; repeated release was idempotent; repeated `from-task` restored the exact observation without a build. |
| CLI bound state: `doctor`, `build --state …`, `observe --state …`, `release --state …` | Doctor admitted after preflight; build was `Success` with exact `out`; observe matched; release removed roots. |
| Preflight failure fixture | Rejected before a rooted build; no success observation was admitted. |

## Observed behavior

- Profile admission requires the repo-pinned `nix` and `nix-store` identities,
  nixpkgs revision, Nix version, exact expected output paths, limits, and
  non-claims. Drift fails before dispatch.
- The adapter runs `nix-store --add-root <effect>/root --indirect --realise
  <drv>` with a cleared environment, fixed `HOME`/`NIX_CONF_DIR`/`NIX_CONFIG`,
  sandbox enabled, no builders or substituters, bounded output, and
  process-group teardown. A bounded `nix-store --realise --dry-run` preflight
  runs before the rooted build.
- One root per expected output is admitted; every target must equal the exact
  expected store path. Missing, malformed, duplicate, or mismatched roots are
  `ProviderError`; nonzero exit is `Failed`; real timeout and cancellation are
  terminal; ambiguous process start is `Unknown` without a terminal receipt.
- Successful products remain rooted until explicit idempotent release. A real
  `NixEvalAdapter` discovery of the pinned fixture flake produced a
  `TaskProposal` that built through `kiln-nix-build`, admitted the expected
  `job-a` output, and released its roots.
- Durable state stores the profile source, plan, observation, and release state.
  An existing observation is restored without rebuilding; an interrupted state
  without an observation is refused; release updates the state idempotently.
- Duplicate dispatch returns the recorded observation; changed plans and
  observation-only restore conflicts fail closed. Log reads are identity-bound,
  bounded, and unavailable after observation-only restore.

## Non-claims

This evidence proves only the exact pinned executables, exact derivation,
local store inputs, exact expected output paths, native realisation roots,
product/receipt/log identities, composition mapping, durable-state behavior,
and reconciliation exercised above. It does not prove build correctness,
sandbox completeness, derivation provenance or source trust, output content
semantics, cache truth, reproducibility, or release readiness.
