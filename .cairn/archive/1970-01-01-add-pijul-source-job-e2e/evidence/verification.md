# Verification: Pijul Source-to-CI E2E

## Source binding

- Parent commit: `b7d4469b0fa7993e306011efcdec145ce6234232`
- Tested working-copy snapshot: `9939e5c1f370` (change `nkuttsyurlpl`)
- This evidence record and the task/spec lifecycle edits were added after that snapshot; they do not change adapter or test behavior.
- `crates/kiln-adapter-pijul/tests/full_e2e.rs` BLAKE3:
  `63e2f01005a86c16af2da8ebed85be4e48d1eb157b5e071e3b7fd2ab2547ad9a`
- `crates/kiln-adapter-pijul/tests/native_repository.rs` BLAKE3:
  `8a1dd245a8a655e7a37cd41aab96d51fcfd4ee45524a87970e6137332e6dca4c`
- `crates/kiln-adapter-pijul/tests/support/mod.rs` BLAKE3:
  `08f777ccca903b150397a483cbe73b1e4fa3f381cad05c785377c90bcefe254b`

The tested snapshot includes the lifecycle delta heading and the workflow-source
projection fix that maps the exact Pijul state identity to a workflow `revision`
reference. The pinned Pijul cohort and the adapter implementation are unchanged
from the parent commit. The archive move and this evidence record followed the
test run and preserve the tested bytes.

## Exact command

```console
nix build path:$PWD#checks.x86_64-linux.pijul-native-fixtures \
  -L --offline --builders '' --max-jobs 4
```

Result: PASS. Output path:
`/nix/store/gk062yy1pzzazs646swgs3j2hp022lmv-kiln-pijul-native-fixtures-0.1.0`.

The Nix check sets `KILN_PIJUL_EXE` to the pinned
`pijul-1.0.0-beta.11` executable and runs:

```console
cargo test -p kiln-adapter-pijul --features native-fixtures --tests --offline
```

## Observed test results

| Target | Passed | Failed |
| --- | ---: | ---: |
| `kiln_adapter_pijul` library tests | 10 | 0 |
| `full_e2e` | 2 | 0 |
| `native_repository` | 4 | 0 |

`full_e2e` covers:

- exact channel resolution and acquisition through `LocalPijulAdapter`;
- materialization of admitted entries only, with exact file bytes, executable
  permissions, no `.pijul` metadata, and rejection of unsafe paths;
- a reviewed Bounded Exec command over the materialized checkout;
- workflow observation admission and `StartRun` / `CompleteAttempt` terminal
  outcomes for success and `DomainFailure`;
- equality between `SourceResolution.exact_revision` and the observed native
  state, and use of that state identity in the workflow source context.

## Non-claims

This evidence proves only the local pinned Pijul cohort, the exact fixture
repository and channel, the reviewed commands, and the selected Kiln
transitions. It does not prove source trust, signature validity, complete
history, remote Pijul availability, collaboration status, host confinement,
provider correctness, release readiness, or authority to publish.
