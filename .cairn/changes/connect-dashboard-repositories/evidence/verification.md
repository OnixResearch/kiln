# Verification: real repository dashboard integration

Observed 2026-10-26. Scope: local replay/export/rendering and the explicit Aspen3
HTML destination. This is not a new hosted CI qualification or release receipt.

## Outcome and ownership

The real offline dashboard now groups seven retained Kiln histories in two
repository declarations, preserves independent canonical observations and local
view selections, and can deliver only the generated HTML atomically. Kiln
maintainers own the dashboard/provider composition; operators own source
freshness, runtime/credential enrollment and delivery destinations. The durable
contribution is reusable bounded catalog admission, exact-profile GitHub replay
export and configured report delivery, rather than a new provider control path.

## Real inputs and claim boundary

The GitHub observations were freshly exported from the retained original-profile
replay database, not synthesized from display labels. The database snapshot was
BLAKE3 `8c771253e146053e38468a889049946676a18ce93c2774640f10b411fa4b23a1`.
The exported profile identity is
`9154f9a232a46f86787b7867aa0c7fa2b98aa1e31e12100c70f2b76d0fe5319f`.

| History | Exact source revision | Retained outcome |
|---|---|---|
| GitHub pass | `e25b7c1a1faac1e9f4eaea24bd9c7cd916ebb817` | Success |
| GitHub genuine assertion failure | `d363f0f6f302da6cde1aa5fb66b64c34ec0cc247` | DomainFailure |
| GitHub controller/bridge recovery | `2bba857fe78760ef6409da4188125dd0bba9a907` | Success |
| GitHub earlier execution | `19b4f0ef56385d3566b5ade94217d683bae84bf9` | Unknown |
| Radicle self-CI success | `4a2a3e6a80fd5c8c5f42e4a0ace6c333d26d4e8a` | Success |
| Radicle genuine Nix failure | `86e35b5e9d191f9992fbc3c5fb85a8c0bf51db16` | DomainFailure |
| Radicle unavailable worker | `995da1ff126ece7bbca91f5fc014bf100e4818f7` | Unknown |

The GitHub duplicate-delivery and controller/bridge recovery facts are retained
in `.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json`:
the duplicate retained run `eb2c42d24935a1141aeab7bcfea097d12c5aef677e79cad91542dc168bda8e1c` without a new worker job, and recovery retained
native execution `b3:9361b7304625aa841da5a9cf1660d8a5bce140084e4af27f8894974cf6d11bb2`
while the executor survived interruption. These provider operations were not
triggered again for this dashboard work. The Radicle histories come from
`.cairn/archive/1970-01-01-qualify-kiln-self-ci/evidence/success/` and identify the
dedicated self-CI fork, not canonical publication. Fresh hosted execution would
require a separately admitted current runtime, credentials and provider mutation
authority; no new push, signing, webhook/status write or service mutation was
inferred from observation work.

The four exported journals under `evidence/dashboard-repositories/` have BLAKE3:

- `github-pass.json`: `3460e05ef55d784242198daa735ab15fa51a0997e83fe408671bc09c3bb59453`
- `github-fail.json`: `f25c75b285d1810a7df5f3823e50c6f0813b79f74c7b9903da1c12fff3b742f2`
- `github-recovery.json`: `b3a9b8b82d7b8c385db82d557493fa4eaabf57b22c9af727560d726ed540e7e5`
- `github-unknown.json`: `3ea9d0deafb50847b47241cd1623254a8c51c08699fa780b1629bc14b3773ece`

Catalog JSON: `c13edd7eeef2c2962d4443206cd9650a0044638b76cbd2db4e9c68af45647e46`.
Dashboard profile: `359c4d0f8a07f926937df605fd5c3aa77fb78f131208ae0fd10a756b5d6fccfb`.

## Runtime verification

- Ran the actual `kiln-dashboard --catalog … --profile … --format html --output …`
  and `kiln dashboard` catalog commands over these real inputs. The final snapshot
  from both entry points has BLAKE3
  `df2321c93a250781cde87adac1a4a8db3d4deb434e56df94c72618037f26ca0e`.
- Drove Chromium history/repository controls: genuine assertion failure displayed
  DomainFailure; recovery displayed Success; the earlier run displayed Unknown;
  Radicle failure displayed DomainFailure. A filter survived history switching and
  reload. A removed history displayed an explicit missing-selection message and no
  substitute report.
- Disabled JavaScript: all seven history links and independently admitted sections
  remained readable. Checked the 390-pixel viewport: outer scroll width was 390,
  with no outer horizontal overflow. Inspected desktop, narrow and no-script
  screenshots. Browser error inspection returned no entries.
- Ran the real publisher against `brittonr@aspen3` with the reviewed destination
  `/home/brittonr/.local/share/kiln/dashboard/index.html`. Remote file owner was
  `brittonr`, mode `600`, size `344399` bytes. Its final snapshot BLAKE3 matched the
  local/CLI value above. No database, journal, profile or credential was transferred.
- Exercised `publish(..., watch=True)`: finite generation delivered a refresh-enabled
  report, BLAKE3 `64c34239904507ebd13b989a235fe8f1c2eb5bf4e85b1ef8e3cf0aca6fabc9ae`.
  Chromium automatically reloaded after the configured 30 seconds and retained the
  selected GitHub Unknown history.
- A malformed profile made actual generation exit 1. An actual SSH/rsync attempt
  to the reserved nonexistent host `kiln-dashboard-unreachable.invalid` exited 255.
  Each failed visibly; the local last-good report remained byte-identical, and the
  Aspen3 report still matched its BLAKE3. The final one-shot delivery restored the
  snapshot artifact described above. No persistent watcher/service was installed.

## Repository checks exercised

- `cargo test --locked --workspace`: passed after correcting the catalog core/shell
  boundary. The first run caught filesystem observation in the catalog core;
  filesystem work now lives in the existing shell and incremental rendering stays
  pure. No boundary allowlist was weakened.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `python3 -m unittest discover -s scripts -p test_dashboard_publish.py`: 9 passed.
- Nickel formatting for new catalog/publication inputs: passed. Both positive
  fixtures and the retained catalog exported successfully; all nine
  `negative-dashboard-*.ncl` inputs were rejected.
- Cairn proposal, design and task-definition gates: PASS. `cairn validate`: valid.
- Default Cairn traceability coverage: failed with no missing accepted requirements,
  but twelve dangling markers from the active dashboard integration and existing
  Patchgate change. The default profile enrolls accepted `.cairn/specs`, not these
  active deltas. Release-readiness correspondingly reported `valid: false` and
  `onix_release_ready: false`. No acceptance sync, archival or policy bypass was
  performed to hide this result.
- Initial packaged Octet check failed before building because the configured
  build-farm hook could not create its overlong `/nix/var/nix/current-load/grpc:…`
  lock-file name, followed by unexpected EOF from the build hook. This is not an
  Octet source-policy verdict.
- Re-ran the packaged Octet gate with `--builders '' --max-jobs 4` to bypass only
  the broken host build-farm route. It passed; the rebuilt Aspen runtime and
  Radicle broker checks each reported clean, with zero findings, warnings or errors.
- During the local flake run, packaged Clippy completed successfully, the native
  Radicle status fixture rail passed 16 tests, and the Nix evaluation process rail
  passed 3 tests. These are isolated fixture checks, not newly operated hosted CI.
- `nix flake check path:. --no-write-lock-file -L --builders '' --max-jobs 4
  --keep-going`: all checks passed on x86_64-linux, including the packaged
  dashboard-publication, Nickel, GitHub protocol-fixture and Bombadil rails. The
  checked implementation cohort was copied to
  `/nix/store/5v2s3yi1056446qplr7jcvqjii3d44kx-source`; subsequent edits only record
  lifecycle task/evidence results. The aarch64-linux checks were not exercised.
- After recording the results, the final Cairn task gate passed with all six
  tasks complete and zero issues; repository validation also remained valid.
