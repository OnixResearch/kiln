# Tasks

## Phase 1: Immutable source and cohort

- [x] [serial] Capture the existing nine-case `status_native` baseline, compare pure status admission tests after cutover, and keep the current source diagnostics and unknown results as acceptance. r[kiln.radicle.status_store_fixtures] r[kiln.radicle.status_store_concurrency]
- [x] [serial] Wait for tested shared v0.2.0 on COB 0.21.1, crypto 0.19.0, and git2 0.21.0; record its actual immutable Git commit, archive BLAKE3, licenses, and archive/import recipe without changing the old v0.1.1 pin. r[kiln.radicle.status_store_pin]
- [x] [serial] Import the verified archive through the Kiln-owned vendor script; reject mismatched commit, digest, source record or imported bytes, track the snapshot and source record, and pin the adapter to that in-repository path and lockfile. r[kiln.radicle.status_store_pin]

## Phase 2: Adapter cutover

- [x] [serial] Replace `status/storage.rs` change/object storage traits, reference walking and ref mutation with the shared borrowed-store binding in both status and patch review transport; preserve ephemeral Git commit configuration lifetime and Kiln diagnostic translation. r[kiln.radicle.status_store_boundary] r[kiln.radicle.status_store_scan]
- [x] [serial] Bind `CiJobStatus` evaluation and shell create/update/get to the shared type, keep pure plan admission, status reducer, SecretSpec signer policy, RID allowlist and provider observations unchanged, and remove obsolete store code. r[kiln.radicle.status_store_boundary]
- [x] [serial] Use the shared positive distinct-ID bound and exact expected-target CAS; retain pre-sign stale admission, no-overwrite first write, `Refs`-to-unknown classification and read-only reconciliation. r[kiln.radicle.status_store_scan] r[kiln.radicle.status_store_concurrency]

## Phase 3: Native verification and evidence

- [x] [parallel] Run real local COB status and patch fixtures for create, exact update, repeat/reconcile, foreign and multi-head admission, stale/moved ref, ambiguity, redaction, missing object, bounded scan and diagnostic precedence with test-only keys. r[kiln.radicle.status_store_fixtures] r[kiln.radicle.status_store_concurrency]
- [x] [parallel] Run shared-store fixtures for zero/overflow bound, duplicate authenticated namespace identity, malformed/symbolic refs, signed-author mismatch and invalid signatures; run Kiln native malformed/author diagnostic fixtures and an altered vendor archive; assert stable Kiln codes. r[kiln.radicle.status_store_scan] r[kiln.radicle.status_store_fixtures] r[kiln.radicle.status_store_pin]
- [x] [serial] Update Kiln status and patch transport documentation plus the guarded source-import procedure with the actual source commit/digest, tests and local-only non-claims. No remote initialization, operator signing, push, sync or hosted-ref mutation. r[kiln.radicle.status_store_nonclaims] r[kiln.radicle.status_store_pin]
- [x] [serial] Run focused tests and full affected Kiln checks plus Cairn validation, proposal/design/task gates and traceability; record concrete results and only then sync accepted specs and archive the change. r[kiln.radicle.status_store_fixtures] r[kiln.radicle.status_store_nonclaims]

## Local evidence

- Shared v0.2.0 `cargo test --offline`: four real signed COB integration tests passed; Kiln `status_native` 12/12, `mutation_native` 6/6, pure `status::tests` 18/18 after the cutover. The prior native status baseline was 9/9. Native status tests invoke the public `RadicleStatusShell::publish` and `.reconcile` against temporary bare Git repositories, observe signed local refs and unknown outcomes, and exercise the vendored `GitCobStore` rather than a simulated CLI.
- Offline Cargo metadata resolves the adapter's shared package from `kiln/vendor/radicle-cob-git-store/Cargo.toml`, version 0.2.0, without a sibling-source dependency.
- The Kiln importer accepted a first exact archive, accepted an identical reimport, and refused a modified file, an executable-bit change, and a mismatched source record in throwaway directories.
- Kiln offline workspace `cargo check`, `cargo test --workspace --features kiln-adapter-radicle/status-native-fixtures` (including the vendored crate's four integration tests), `cargo clippy --workspace --all-targets --features kiln-adapter-radicle/status-native-fixtures -- -D warnings`, and `cargo fmt --all -- --check` passed.
- Cairn strict validation reported `valid: true` for the Kiln lifecycle store. Proposal, design, and tasks gates each reported `PASS` after scope correction.
- The earlier global Cairn traceability check reported `valid: false` due to 22 then-missing `kiln.celld.*` scenario markers and three then-dangling `kiln.celld.*` markers, outside this status cutover. At that point task 10 remained intentionally unchecked and sync/archive were withheld rather than bypassing the gate or altering unrelated Celld ownership.
- After the Celld Worker repair supplied runnable scenario evidence, global Cairn traceability reported `valid: true`, 561/561 referenced, no missing or dangling markers. The offline full workspace native-fixture test suite, strict all-target Clippy with `-D warnings`, and Cargo format check passed; both packages can now use the native lifecycle gates without weakening accepted requirements or touching the unrelated active evaluator cohort.
