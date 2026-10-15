# Public-release preparation verification

## State and exact inputs

The prepared source payload passed local validation and was applied to the
original checkout with preserved-baseline guards. The requested `datapool/git`
quota increase from `800G` to `820G` removed the write blocker. Original-checkout
dependency resolution and lifecycle gates passed. **Nothing was published.**

Kiln maintainers own this repeatable preparation path for outside contributors.
The durable capability is credential-independent source acquisition, a diagnosed
and reproducible Pijul producer recipe, and scoped distribution notices—not a
new claim of production readiness or publication authority.

[`source-manifest.json`](source-manifest.json) identifies the exact 721-file input
tree used by the final Nix check. Its BLAKE3 is:

```text
9b92e0b0c117bc9e3f26260e85358d3e26eebe5dbb18fe981eafb1fbb5f629a4
```

Hash the exact UTF-8 file bytes: compact, sorted-key JSON, no trailing newline.
Each regular-file entry binds its relative path, byte count, executable bit, and
BLAKE3; symlinks bind their target. This is a source-payload identity, **not** a
Git revision or canonical publication archive identity. This evidence and later
lifecycle-completion bookkeeping were recorded after validation and are not silently
included in that checked input.

[`observations.json`](observations.json) retains commands, outcomes, receipt
identities, acquisition scope, failures, and non-claims. The earlier workspace
run used manifest identity
`5a5fc98bbad26150b9dca901d95a0d6724de5cdc916473e1b2ce3afc6dac7c68`.
Only `flake.nix` and `docs/reference-pins.md` changed before the final Nix run:
the formatter invocation and its explanation. Before/after manifest entries are
retained so the earlier input identity can be reconstructed without duplicating
the entire manifest. No Rust behavior changed between those runs.

## Implemented repairs

- Six scoped, pristine producer imports retain exact commits, selected archive
  paths and BLAKE3, full upstream archive identities, and applicable licenses.
  Direct and transitive Cargo inputs and Cairn/Mantle producer fixture checks no
  longer require private Git access or sibling checkouts. Locks were regenerated
  by Cargo and Nix, not hand-edited.
- `scripts/import-pinned-source.sh` accepted every exact import and identical
  reimport. A modified existing destination was rejected without replacement.
  Upstream Lattice, Cairn, and Mantle fixtures/contracts still match the frozen
  Kiln cohorts.
- The existing stock Onix rejection fixture references accepted
  `kiln.nix_eval_jobs.onix_boundary`, not draft
  `kiln.nix_eval_jobs_onix.fallback`. Its predispatch rejection remains intact;
  unfinished Onix evaluation is not promoted to an admitted capability.
- `LICENSES.md` and retained license texts distinguish permissive Kiln source,
  GPL Pijul, linked AGPL Bounded Exec, MPL Valence, and other imported material.
  Owner-specific reuse authorization is not extended to public recipients.
- Current README/operator/hosting documentation distinguishes historical
  publication evidence from newly exercised local checks. Original publication
  and release-evidence files remain unchanged.

## Exercised checks

| Check | Observed result |
| --- | --- |
| Fresh `HOME`/`CARGO_HOME`, credentials disabled, `cargo fetch --locked` | PASS; 372 public registry archives downloaded, no Cargo Git cache |
| Offline Cargo metadata with only public registry cache reuse | PASS; imported dependencies resolve locally, zero resolved Git packages |
| Fresh Nix chroot store, empty HOME/XDG caches, no credentials or substituters, `nix flake archive` | PASS; initially zero store paths, all 8 root/transitive flake nodes acquired |
| `cargo test --locked --workspace` | 329 passed, 0 failed; 2 opt-in live Celld tests ignored |
| Strict all-target workspace Clippy | PASS with warnings denied |
| `kiln-core` and `kiln-radicle-source-contract` for `wasm32-unknown-unknown` | PASS |
| Pijul native repository fixtures | 4 passed, including exact states, distinct histories, and fail-closed cases |
| Stock evaluator process fixtures | 3 passed, including cancellation, late output and descendant teardown |
| `node --test tests/celld-worker.test.mjs` | 16 passed; signed Worker/real-SQLite fixtures |
| Cairn policy positive/negative fixtures | PASS |
| Cairn validation and proposal/design/tasks gates | PASS under the repository's explicit policy |
| Traceability | 567/567 accepted requirements referenced, no missing/dangling markers |
| Workspace-member formatting and corrected Nix `format` check | PASS |
| Full `nix flake check` after the formatter fix | All `x86_64-linux` checks passed |
| Final packaged `kiln demo` | Revision 1, `demo-start` accepted, run and attempt `Pending`, no holds |

The full Nix check used only `https://cache.nixos.org` as a binary substituter
and disabled remote builders. It reused the normal system store. The separate
cold-store smoke proves flake-source acquisition without that cache reuse; it
does not claim a source bootstrap of every toolchain/native build dependency.
`aarch64-linux` was omitted as incompatible on this host, not reported as built.
No live Celld service deployment or exhaustive history secret scan is claimed.

## Pijul drift diagnosis and reproduction

Two builds of the unpatched producer had executable BLAKE3 identities
`4d8181701c8bbfe4a1d7eb81d132457aaf90077b390b9aa0c71a513d907ffbfe`
and `ef2e126341a66a6b26e9fc310b606f4d177213cf2a56b96a8159f83adef5b797`.
ELF inspection found the same symbols and sizes but reordered Zstd functions.
`zstd-seekable` 0.1.23 feeds unsorted `std::fs::read_dir` results into
`cc::Build`, so filesystem enumeration order changed native archive member and
executable layout.

The retained patch sorts the existing paths only in the Nix build's writable
vendor copy. Original crates.io inputs and library versions remain pinned.
A fresh build followed by `nix build DRV^* --rebuild` completed successfully;
Nix compared the complete outputs before the new identity was admitted:

```text
DRV: /nix/store/079qfghzrcddfanpw2ckfbzkgrvfjwff-pijul-1.0.0-beta.11.drv
EXE: /nix/store/qr2sja5n9r9m0j9q2pyn60cyb6lg3vnp-pijul-1.0.0-beta.11/bin/pijul
BLAKE3: 9b8d58a1ced03ceb73347e4ed4818d17cdbff2de66d9024d4ec9b87327a4f934
```

This is two clean sandbox builds on this host, not universal or cross-host
reproducibility. Exact path, executable hash, version, immutable source checksum,
and producer lock-cohort checks remain enforced.

## Retained failures and scope correction

The first Cargo format invocation and first integrated flake run failed because
`cargo fmt --all` traversed path dependencies into intentionally partial
upstream workspaces. The original Cargo error is retained in
[`initial-format-failure.txt`](initial-format-failure.txt); it is not relabeled
PASS. The corrected invocation checks every admitted workspace member,
including the existing shared COB store, without importing unrelated producer
applications or modifying pristine upstream manifests. A verbose formatter run,
the corrected Nix formatter gate, and the repaired full flake check then passed.

An initial zero-fuzz native-source patch attempt also rejected its incomplete
unified-diff trailing context. That patch was corrected before the successful
build/rebuild; no failed artifact was admitted as a cohort.

## Original-checkout application and authority

All 185 planned operations passed the preserved-baseline guard before application;
unrelated changes were preserved. The six producer imports used the repository's
drift-refusing import script. Cargo and Nix regenerated their lockfiles in the
original checkout; both exactly match the verified prepared copies.

Comparison with the checked 721-file manifest found no product/build input
differences. Subsequent lifecycle-completion edits to the preparation design and
task list, and the evidence recorded here, are bookkeeping—not a claim that the
earlier Nix run included its own finalized receipt.

Offline Cargo resolution in the original checkout admitted 24 workspace members
and 403 packages with zero Git-sourced packages. Strict Cairn validation,
traceability (567/567, no missing or dangling markers), and proposal/design/tasks
gates passed under the explicit repository policy. Final commands, exact receipt
identities, lockfile identities, and lifecycle input differences are retained in
[`observations.json`](observations.json). The full workspace/native/Nix checks
above were exercised in the isolated checkout; they are not relabeled as a second
full run in the original checkout.

No Radicle signing, push, synchronization, announcement, mirror publication,
governance mutation, specification sync, or lifecycle archive was performed.
The historical receipt for `36984e367eafd44c22d2b4f4f47121deb51eece3`
retains its original meaning. A new canonical publication needs separate
operator authorization and a consistent exact-revision receipt.
