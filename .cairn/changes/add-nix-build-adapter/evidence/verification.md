# Verification: Exact Nix Build Provider

Date: 2026-10-19
Workspace: `/home/brittonr/git/OnixResearch/kiln`

## Pinned cohort

- Nix version: `2.36.0`
- executable: `/nix/store/6rwk5j1qqk7na4la5m2ka34p734braxa-nix-2.36.0/bin/nix`
- executable BLAKE3:
  `0e670a46c789a33272d9193080b91affe8023af27bcfa9fad76d3d959178a8e7`
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525`

## Commands and outcomes

| Command | Outcome |
|---|---|
| `cargo test -p kiln-adapter-nix --offline` | 13 fixture tests passed. |
| `cargo test -p kiln-adapter-nix --features nix-live --offline live_nix_build_is_admitted -- --nocapture` | Real pinned Nix derivation was realized by the pinned CLI, admitted as `Success`, and the `out` content was `live\n`. |
| `cargo clippy -p kiln-adapter-nix --all-targets --offline -- -D warnings` | Passed. |
| `cargo test -p kiln-conformance --offline` | Passed; the adapter boundary audits include `kiln-adapter-nix` and the core dependency audit rejects it. |
| `cargo fmt -p kiln-adapter-nix --check` | Passed. |
| `nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-fixtures -L --no-write-lock-file` | Passed; the sandbox ran the 13 fixture tests and did not run the real Nix process. |
| Final corrected-source check result | `/nix/store/gsm4jrbngsn4gy2k3qg9gq8ghfi84il5-kiln-nix-build-adapter-fixtures-0.1.0` |
| `nix run nixpkgs#nickel -- typecheck config/nix-build-profile.ncl` | Passed. |
| `nix run nixpkgs#nickel -- format --check config/nix-build-profile.ncl config/fixtures/nix-build-positive.ncl` | Passed. |
| `nix run nixpkgs#nickel -- export config/nix-build-profile.ncl` | Passed. |
| `cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"` | Passed with no issues. |
| `cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json` | Passed; 678 requirements referenced, no dangling or missing markers. |

## Observed behavior

- Exact profile admission succeeds; changed Nix version, executable identity,
  derivation identity, capability set, product order, and inconsistent limits
  fail before dispatch.
- Exact plan binding succeeds; changed provider, plan, policy, platform,
  products, or unsafe derivation fail before a process starts.
- The live local build used `nix build --no-link --json --offline --expr
  'import <exact .drv>'` with a cleared environment and fixed `HOME` and
  `NIX_CONFIG`; remote builders and substituters were disabled.
- Zero exit with every required output is `Success`; nonzero exit is `Failed`;
  timeout and cancellation are terminal; malformed, missing, outside-store,
  signalled, and output-limited results are `ProviderError`; ambiguous process
  start is `Unknown` without a terminal receipt.
- Duplicate dispatch returns the recorded observation without a second process;
  changed plans and observation-only restore conflicts fail closed.
- Log reads are bound to the exact effect and identity, bounded by the profile,
  and unavailable after an observation-only restore.

## Non-claims

This evidence proves only the exact pinned executable, exact derivation,
local store inputs, native Nix JSON, product/receipt/log identities, and
reconciliation behavior exercised above. It does not prove build correctness,
sandbox completeness, derivation provenance or source trust, output content
semantics, cache truth, reproducibility, or release readiness.
