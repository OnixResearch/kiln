# Isolated native Radicle reducer

This separate workspace pins `radicle = 0.25.1` and uses its native issue and
patch reducers/permission rules. It isolates upstream `sqlite` from the root
workspace's incompatible `rusqlite` native linkage. No Radicle provider crate
enters Kiln core.

The selected helper compiler cohort is Rust **1.97.1** (`8bab26f4f`, stable).
This is a tested selection, not a claim of the true minimum supported compiler.
The root workspace retains its own Rust 1.88 cohort. An observed build of
upstream radicle 0.25.1 with Rust1.88 failed at `cob/identity.rs:506` with E0382
(the `action @ RevisionAccept | RevisionReject` pattern), before helper code.
Do not patch upstream or relax its pin to conceal that prerequisite.

```console
cargo +1.97.1 test --locked --offline --manifest-path tools/radicle-patch/Cargo.toml -- --nocapture
cargo +1.97.1 build --locked --offline --manifest-path tools/radicle-patch/Cargo.toml
```

The Nix-native fixture gate pins that helper-only compiler independently of core:

```console
nix build path:.#checks.x86_64-linux.radicle-mutation-native-fixtures --no-link -L --builders '' --option build-hook ''
```

This gate runs the real upstream reducer/permission and multi-writer CAS fixtures;
it no longer invokes the obsolete synthetic `mutation_native` test target.

Set `TMPDIR=/tmp KILN_RETAIN_RADICLE_FIXTURE=1` for the test command only to retain
the disposable native fixture. Its final JSON line names the storage root,
RID, patch/revision, issue and observed heads for direct CLI read/review smoke.
Without this explicit request the fixture is removed automatically. Retire any
retained directory after local proof; fixture signing keys are not real identities.

Runtime commands:

```text
kiln-radicle-patch read STORAGE_ROOT REPOSITORY_ID PATCH_ID
kiln-radicle-patch read-issue STORAGE_ROOT REPOSITORY_ID ISSUE_ID
kiln-radicle-patch review STORAGE_ROOT REPOSITORY_ID PATCH_ID REVISION_ID accept|reject SUMMARY EXPECTED_HEADS_JSON
```

`STORAGE_ROOT` uses the native directory layout, e.g. a `rad:z...` repository
lives in `STORAGE_ROOT/z...`, not `STORAGE_ROOT/rad:z...`.

Read commands perform no writes. They enumerate all heads, bound the reachable
history before native reduction, and verify every change signature. `read`
includes exact revision ids, bases, oids, authors, reviews and merges, the native
lifecycle state, the observed head set, delegate identities, and per-revision
signed-author reachability evidence. Reachability accepts an author-signed
branch or patch COB reference; the COB's related Git parents retain exact
revision commits. An unsigned namespace ref is never evidence.
All COB head mappings themselves must match verified namespace signed refs and
tip authors. All history branches must reach the declared root; unknown action
encodings fail closed. A post-reduction authenticated-head comparison prevents
mixed-snapshot projections.

The review command requires `RADICLE_SIGNER_SEED` in its child environment,
never argv, and compares `EXPECTED_HEADS_JSON` immediately before native append.
The adapter resolves only its own declared signer, passes its current head set,
and handles uncertain child termination through read-only reconciliation.
Native mutation uses an isolated bounded ref snapshot sharing immutable Git
objects. Actual publication locks the signer's observed namespace and compares
expected-old refs before publishing COB and sigref tips in a Git transaction.
This CAS protects against external same-signer writers as well as cooperating
helper processes; stale snapshots never replace a winner's review.
No command synchronizes, pushes, announces, changes delegates, or mutates remote
signer state. Invoking local review itself requires operator authority; use
only disposable fixture keys in tests.

The process boundary enforces a cleared environment, bounded stdout/stderr and
process-group timeout in the adapter. The helper itself has absolute bounds of
65,536 entries, 512MiB action bytes, 256 refs/parents and one MiB per commit's
metadata; the adapter normally narrows history to 1,024 entries/16MiB before
calling it. ODB blob sizes are checked before loading payloads. These bounds
are not a claim of hostile-source runtime qualification.

`indexmap` enables `std` because the pinned upstream crate disables it but uses
the default-hasher two-argument `IndexMap` form. The helper emits observation
only: it confers no Kiln review, CI, merge or release authority.
