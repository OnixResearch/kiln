## Phase 1: Coverage contract

- [x] [serial] Add the closed protected-operation coverage map with its Basalt operation ids, owners, effect classes and candidate rule, plus positive and negative tests. r[kiln.implementation.authority.coverage]

## Phase 2: Cohort and import

- [x] [serial] Upgrade `vendor/ucan` to the Basalt cohort `417f74662b9de2de3bdabe93ad5db996e1bd2861` and refresh its source record. r[kiln.implementation.authority.cohort]
- [x] [serial] [depends:kiln.implementation.authority.cohort] Add the repo-owned Basalt import with its manifest rewrite, source record and license entry. r[kiln.implementation.authority.cohort.scenario.import]

## Phase 3: Authority port

- [x] [serial] [depends:kiln.implementation.authority.coverage] Add the typed authority request/decision port in core. r[kiln.implementation.authority.binding]
- [x] [serial] [depends:kiln.implementation.authority.cohort.scenario.import] Add the `kiln-adapter-basalt` crate with positive and negative fixtures over the packaged Basalt policy. r[kiln.implementation.authority.enforcement]

## Phase 4: Enforcement

- [x] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for claim acquire/renew and dispatch at the implementer admission boundary. r[kiln.implementation.authority.enforcement.scenario.absent]
- [x] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for candidate submission, verification admission and cancellation. r[kiln.implementation.authority.separation]
- [x] [serial] [depends:kiln.implementation.authority.enforcement] Require exact authority for publication, integration and lifecycle completion, keeping each grant separate. r[kiln.implementation.authority.separation.scenario.widen]
- [x] [serial] [depends:kiln.implementation.authority.enforcement] Revalidate before each effect and block cancelled, superseded or revoked attempts. r[kiln.implementation.authority.currentness]

## Phase 5: Compatibility and evidence

- [x] [serial] [depends:kiln.implementation.authority.enforcement] Add the explicit local-operator profile with recorded provenance and no role-only or fallback path. r[kiln.implementation.authority.compatibility]
- [x] [serial] [depends:kiln.implementation.authority.currentness] Record bounded redacted decisions with principal, work/attempt/fence, policy, operation and denial reasons. r[kiln.implementation.authority.enforcement.scenario.replay]
- [x] [serial] [depends:kiln.implementation.authority.compatibility] Update the operator guide, changelog, license map and run the focused, workspace and Cairn checks. r[kiln.implementation.authority.cohort]

## Cohort resolution

The earlier blocked-prerequisite analysis measured Basalt with its default
features. Two probe results replaced that plan:

1. `kiln-adapter-basalt` depends on `basalt` with `default-features = false`.
   The `oidc-exchange` and `durable-authority` features, and therefore
   `bounded-http`, `replay-ledger-core` and `revocation-view-core`, never enter
   Kiln's resolved graph. A fresh 1.88 lockfile and check confirmed their
   absence.
2. The remaining declaration blockers were `detect-coding-agent@0.1.1`
   (declares 1.91) through the mandatory `secretspec@0.17.0` dependency, and
   `uuid@1.29.3` in a fresh lock. Kiln's existing `uuid@1.26.1` lock entry
   satisfies SecretSpec's `uuid = "1"` requirement and is retained.

The cohort target was also stale: Basalt actually vendors UCAN
`417f74662b9de2de3bdabe93ad5db996e1bd2861` (the `c483c7b` drain tip is an
ancestor), so Kiln imports that exact revision. The import is byte-identical;
the durable feature stays disabled and its optional revision is not resolved.

Evidence:

- `scripts/import-pinned-source.sh ucan ../ucan` imported a 1,509-entry
  selection and a repeat run verified the same bytes.
- `scripts/import-pinned-source.sh basalt ../basalt` applied the three recorded
  UCAN path rewrites and a repeat run verified the same bytes.
- `vendor/detect-coding-agent.source.json` records the exact 0.1.1 archive
  (SHA-256 `be8df8d4...`, BLAKE3 `3c2a487d...`) and its single
  `rust-version` declaration change (1.91.0 to 1.88.0).
- `cargo check -p kiln-adapter-basalt` and the adapter's six fixtures pass on
  the repository's Rust 1.88 pin.

## Verification

- `cargo fmt --all --check`: pass.
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`:
  pass in the repository dev shell. The one vendored Basalt dead-code warning
  is upstream and is not part of the workspace lint gate.
- `cargo test -p onix-kiln-core --lib authority`: 21 pass, including the local
  profile, bounded request/decision and redaction cases.
- `cargo test -p kiln-adapter-basalt`: 6 pass, including the packaged-policy
  coverage, missing-contract denial, local profile, projection denial,
  malformed-token/no-replay and cancelled-currentness cases.
- `cargo test -p kiln-implementer --test admission --test durable` and the
  implementation unit tests: pass. `cargo test -p kiln-fleet --test dispatch
  --test service`: pass.
- `cargo test --workspace`: pass. Two pre-existing wall-clock fixtures were
  made load-robust: the structural preview keeps its reported 2-second contract
  but uses a 30-second matching bound in unit tests, and the worker deadline
  fixture waits for the expected queued request count and uses scaled timings
  with a 10-second liveness bound. `cargo test -p kiln-web-service --lib`: 284
  pass, 2 ignored, in parallel under concurrent workspace builds.
- `cairn validate --root .`, `cairn policy export --check`, and the
  proposal/design/tasks gates: pass.
- `nix build .#checks.x86_64-linux.{format,nickel}` were cancelled after
  waiting on another session's Nix garbage-collector/build lock. The direct
  equivalents passed: `cargo fmt --all --check`, `nickel typecheck` for the
  fleet and campaign contracts, the positive fleet export, and the negative
  fleet fixtures.
