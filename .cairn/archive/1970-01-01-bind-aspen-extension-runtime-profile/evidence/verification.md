# Verification evidence

Date: 2026-08-28

## Live defect

The paced production closure admitted runtime profile identity `da2996d3637e4ba693a234e713d464bfa06daeec5f32b2aa40421bff50e5f725`, whose callback timeout is `7,290,000` milliseconds. The deployed extension still admitted its compiled fixture, whose timeout is `30,000` milliseconds. The prior live profile identity `2008b2d661855cf7a384706b6641d8baf697bb66dc78a1a3fa97322a9e6be723` had matched that fixture.

The fresh status-disabled shadow reserved operation `44548a9302505778d3e43384a67be779ffa201e30ba0fea8d23ada664d4b7bf2`. Its durable record remained `reserved`, with no callback receipt, effect intent, provider observation, acknowledgement, Lattice operation, or report. The adapter received `aspen_ingress_malformed`. This is pre-provider defect evidence, not provider-backed canary evidence.

The exact trigger used actor `did:key:z6MkmkA6sEzzMffaWqKEKJcDh8LjAzgrJLrTNi971KN3X6sh`, before revision `d88cc41b0145d5dc118a6313054c5d3e66efbe19`, and after revision `5f659dce24e13b30e996f0aab3419dac4c21f934`.

## Repair

The host now canonicalizes the admitted profile path and binds this explicit argv into each extension execution:

```text
--runtime-profile <canonical-path>
--runtime-profile-identity <admitted-blake3-identity>
```

The extension shell reads the document under a one-MiB bound, admits it, compares the resulting identity, and only then reads and handles the callback frame. The deterministic callback core still receives an admitted profile and performs no file or environment access.

## Pre-change evidence

- `cargo test -p kiln-aspen-extension`: PASS, 7 integration tests.
- `nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L`: PASS.
- Native Cairn validation and proposal, design, and tasks gates: PASS.

## Changed-surface evidence

- `cargo test -p kiln-aspen-extension --all-targets --all-features`: PASS, including three new shell tests and four hosted tests.
- `cargo clippy -p kiln-aspen-extension --all-targets --all-features -- -D warnings`: PASS.
- Root workspace tests with all targets and features, excluding only the separately gated Pijul native fixture: PASS.
- Root workspace strict Clippy with all targets and features: PASS.
- Standalone host workspace tests with all targets and features: PASS, 19 tests. The new negative test proves profile drift after reservation remains `Unknown`, repeats without Lattice dispatch, and fails closed on restart.
- Standalone host strict Clippy with all targets and features: PASS.
- Root and standalone host formatting checks: PASS.
- `checks.x86_64-linux.aspen-runtime-profile`: PASS.
- `checks.x86_64-linux.aspen-runtime-octet-deny-all`: PASS with zero findings, warnings, or errors.
- `checks.x86_64-linux.kiln-aspen-host-package`: PASS with release-mode build and all-target, all-feature tests.
- `checks.x86_64-linux.kiln-aspen-host-octet-deny-all`: PASS with zero findings, warnings, or errors.
- Native Cairn validation: PASS.

The full all-feature root command reached only the existing native Pijul tests without `KILN_PIJUL_EXE`; every preceding test passed. The malformed-Cargo diagnostic comes from Aspen's intentional negative fixture. Neither diagnostic belongs to this changed surface.

## Fresh status-disabled Onix shadow (2026-10-20)

An isolated shadow host used the deployed Onix runtime profile and real Lattice/Nix provider. This was a synthetic local `Push` request for an already present Radicle revision, not authenticated forge delivery or a production service cutover. The shadow Radicle profile enabled only trigger intake; source, collaboration, and status capabilities were disabled. Its replay database and host state were separate from production under `/var/lib/kiln-aspen-radicle-ci/shadow-20261020/`.

The exact exercised cohort was:

- host: `/nix/store/mpwqscmfwqym271f0wi0816xl4b3sc6z-kiln-aspen-host-0.1.0/bin/kiln-aspen-host`, executable BLAKE3 `bee99e65e74048be6a6aaf37f5ad767a996f50f87e5d446f03bb85925ad54957`;
- extension: `/nix/store/1vlin9jfjyxhn3gaaviph0c9lqrgwd59-kiln-0.1.0/bin/kiln-aspen-extension`, executable BLAKE3 `963a826d12603417f64fc0b0e36c1358e611283601aab123e3e4749be128e537`;
- Lattice: `/nix/store/mwqnyq525wpxn0jw6w7c06fq41l98z9m-lattice-0.1.0/bin/lattice`, executable BLAKE3 `4c3773e852f6b66cb33677e74899d37a2bed0dcf4742698c40d12d0813783aba`;
- runtime profile: `/etc/kiln-aspen-ci/aspen-profile.json`, file-byte BLAKE3 `bc0e437401435003b75d9c4790292a687dc797650ff68d2ad1816a233ad9e3ad`, admitted semantic identity `da2996d3637e4ba693a234e713d464bfa06daeec5f32b2aa40421bff50e5f725`, callback timeout `7,290,000` milliseconds;
- host arguments: `--max-requests 16 --timeout-ms 7300000 --poll-interval-ms 7068`; the Radicle profile retained a maximum of `1,023` polls and the real endpoint `/run/kiln-aspen-ci/internal/lattice.sock`.

### Completed terminal-failure observation

The request used repository `rad:z3xXXCQXCTquvAawh41YYs8yC8xmk`, branch `master`, before revision `fce00eb4106d8732e178635f241b5be3a027e828`, and after revision `26cf175c7fc6c303851b476d94155f88bccc96e2`.

- trigger identity: `f433260e68e906a8ede6e7ea24b1372eba458a4fe00201ea52ba9db185c4bc79`;
- operation: `60fe022c13e076c8309ec4b0f334165b2206c7ece1674612e89a309ecd67dd5d`;
- Kiln run: `kiln-aspen-run-85eb489008b22149d588c0026e405544e6b8fc96281cb5a8a76b94db587cd563`;
- Lattice execution: `b3:87b8bf217964f7c0e9de486088f6a2e4e876205c2fe8b8ce3d95c9e116c44de8`;
- persisted provider observation: sequence `2`, terminal `Failed`, outcome `DomainFailure`;
- terminal acknowledgement: `Failed`, receipt `0c05233f9f3009d9bd79673a6b63ec3fcd4319583fcaf528682b0cdef4f93b98`.

The real Nix provider published report `b3:3d0168547a6bfd25f02dfe96635e4433587daa6969b79a4859bf76ddefb7dba1` with exit code `1`. The report and log are retained at `/var/lib/radicle-ci/reports/aspen/seaglass/3d0168547a6bfd25f02dfe96635e4433587daa6969b79a4859bf76ddefb7dba1.{json,log}`, both mode `0640`:

- report: `960` bytes, BLAKE3 `d59869ef892cc45edc26ebdb14f0a4cd2121435fdfacca8c78d17dff0139f1d2`, modified `2026-10-20T16:35:13.957746+00:00`;
- log: `84,121` bytes, BLAKE3 `996c2e9a52d6e23f309a6f42985ecb876571e216e3357d7ec5513fbd67404953`, modified `2026-10-20T16:35:13.948745+00:00`.

The log records a fixed-output hash mismatch for `components-ccdb07f`: expected `sha256-s9fz06oVni9eSSmYbPzsDuJdKmqOT8il0DcTjdnZMwU=`, observed `sha256-tsKmcyaScMDIQwdFCHCBulOMTnWZSQD4IxVdXFqMW3Y=`. Kiln preserved this provider failure rather than treating a report or client process exit as successful CI. The non-default runtime profile reached real provider execution and terminal callback admission; the old `aspen_ingress_malformed` failure did not recur. These observations do not qualify the failing Seaglass revision or establish publication provenance for the tested executable bytes.

### Completed success observation

A second, distinct trigger used before revision `26cf175c7fc6c303851b476d94155f88bccc96e2` and after revision `b22335f843fe2bae0d3951ddc7aabaf008df63f4`, with the same repository, branch, runtime profile, and frozen executables. It did not retry or redispatch the failed operation. The isolated Radicle profile's file-byte BLAKE3 was `4b254f3b9b48ac887e66940e117b70a38952231f9d96354b3866543728570b73`.

- trigger identity: `7b7d17649620c30897c36d322104b9fa9e26a06e6ceb88f9f5ced64eca4c6575`;
- operation: `c06fdaf09af704c42a7ccbcaf54a1320ea327a8c14b6696d488379be2921f71f`;
- Kiln run: `kiln-aspen-run-bac0f8a81f5f5d7175e622370e42dce3348ffaaa3dcc51534a3b3dca447bc26e`;
- Lattice execution: `b3:a42fb21e42ffd0e7ad6e780c8ee8a2622f5cd61efb73f9bd8bc4ef3cfeb937e3`;
- persisted provider observation: sequence `2`, terminal `Completed`, successful `true`, outcome `Success`;
- terminal acknowledgement: `Completed`, receipt `0ee544fe0bb6be10c91fe45484e6fdecf2e65719788a324d63fcee1065b67f98`.

The real provider published report `b3:0e3eb707f64cdf0f8deebfc3e4a0570f32a272e411445590d902ab5052d48084` with outcome `success`, exit code `0`, and wrapper BLAKE3 `ff63b7f666ecd2ad2483b75ee627dbda1bf7a104e16795eca7ec447674437cb0`. Its report and log remain at `/var/lib/radicle-ci/reports/aspen/seaglass/0e3eb707f64cdf0f8deebfc3e4a0570f32a272e411445590d902ab5052d48084.{json,log}`, both mode `0640`:

- report: `960` bytes, BLAKE3 `916efe39e0d87cf3dce118950d976fa29d11f7a4085fda54763bc71d85f6b204`, modified `2026-10-20T17:45:47.152817+00:00`;
- log: `197,773` bytes, independently hashed BLAKE3 `57485ad8a36fe41a01d3631523acb9130800d7e5415c1f8ad62291808a4ef93e`, matching the report, modified `2026-10-20T17:45:47.140816+00:00`.

The captured client invocation lasted `4,157.81` seconds and returned the matching terminal acknowledgement. The deployed non-default callback timeout and paced host therefore covered a real delayed provider completion. No exact observe-call count or performance guarantee is claimed.

### Retention and verification

Exact provider report bytes and both terminal operation-record bytes are retained in [`onix-shadow-2026-10-20/`](onix-shadow-2026-10-20/), named by their provider-run or operation identities. BLAKE3 checks of both repository-held provider reports match the original report files above. Full provider logs and original host state remain retained on the operator host; the log hashes are not substituted for successful outcome admission.

After both terminal records were collected, the isolated shadow host container was stopped and removed. The throwaway trigger example was removed from the source tree. The production Lattice service, production host state, and retained failed operation were not replaced or redispatched.

Strict Cairn validation passed with no findings. Traceability coverage passed for all `763` requirements with no missing or dangling references. This closeout changed retained evidence and lifecycle/traceability inputs, not runtime implementation. It did not rerun the historical Cargo, Clippy, Octet, or Nix build gates listed earlier or claim them for current source.

## Published source binding

The exercised host derivation is `/nix/store/baw3akwkqsk6j3hg9fjp94hydlg3x2xs-kiln-aspen-host-0.1.0.drv`, with source `/nix/store/yhkwaw4rbs0skx9nyh6hr93mbrq2za8g-kiln-aspen-host`. The extension package derivation is `/nix/store/rxppwsq0sfgkcww6lxapw9q7nmfchh2a-kiln-0.1.0.drv`, with source `/nix/store/3m8qld7mn3j4za8lp58637wzxldrjlhk-source`.

The exercised cohort is the existing published immutable revision `8c9338e5c10a0e16ee3042d11583ccccf6efe7e9`. Nix metadata for that exact revision resolves to the same source `/nix/store/3m8qld7mn3j4za8lp58637wzxldrjlhk-source`, with NAR identity `sha256-OP24vnc8olwz52VGh4e3dO+g/y7cT8possehEAmv6uE=`. Its `runtime/kiln-aspen-host` subtree has the same NAR SHA-256 `01j9hs3b85vdj5dgnvqqq5yv32c969kfws35928yrq59wzl8vaj5` as the exercised host source directory. Re-evaluating that revision reproduced both derivation paths above exactly; the output is retained in [`evaluated-derivations.json`](onix-shadow-2026-10-20/evaluated-derivations.json).

Read-only acquisition from `git+https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git?ref=main&rev=8c9338e5c10a0e16ee3042d11583ccccf6efe7e9` succeeded in a fresh temporary `HOME` and `XDG_CACHE_HOME`, with global/system Git configuration disabled, no SSH agent, and explicit Git protocol v0. The remote transferred Git objects; the acquired revision, NAR hash, and source path matched the frozen build input. Exact Nix metadata is retained in [`public-source-acquisition.json`](onix-shadow-2026-10-20/public-source-acquisition.json). This establishes acquisition of the declared HTTPS projection's source bytes, not a fresh dependency build, signed-ref authentication, or source trust.

The exercised host's historical Cargo lock binds its Kiln adapter/core dependencies to `9ad8447f8025331917cf5c40ad956a8df40d225a`; it is not the later maintained standalone cohort pinned to `69afc2697f4330005797256135ae81b2722a125c`. Initial comparisons against that later revision and `697f8d00018b56f0d4960dd44288f397b31d9ae1` correctly produced different derivations. Those mismatches do not invalidate the recovered exact `8c9338e5c10a0e16ee3042d11583ccccf6efe7e9` binding, and this shadow is not relabeled as validation of either newer cohort.

The immutable-publication and fresh-live-shadow criterion is satisfied by the already published exact cohort and the retained terminal observations above. No new Radicle publication, synchronization, signing, status write, or production cutover was performed by this exercise. Kiln host maintainers own this repeatable qualification rail and its evidence; operators retain deployment and publication authority. Any new candidate must use distinct ingress/state identities rather than redispatch either retained operation.

## Lifecycle closeout

Proposal, design, and tasks gates passed for both runtime-profile binding and paced observation. Native Cairn sync accepted both requirements into `.cairn/specs/aspen-host-bridge/spec.md`; native archive retained the completed packages as `1970-01-01-bind-aspen-extension-runtime-profile` and `1970-01-01-pace-aspen-lattice-observation`. Peer evidence links were migrated to those archive paths.

The Nickel traceability profile now reads these requirements from accepted specifications only. Its regenerated semantic policy hash is `625a2230165be3edbdd47050e5e070540892852f01ae0fbdbe7d31ca279bc649`. The maintained agent-completion fixtures were rebound to that policy; recomputing all five command hashes and the selection hash through the producer's canonical-JSON/BLAKE3 rules left those identities unchanged. These remain conformance fixtures, not newly executed build receipts. Final native closeout reports are retained in [`post-closeout-checks.json`](onix-shadow-2026-10-20/post-closeout-checks.json).

A separate pinned-shell `nickel format --check` rejected the existing `let`/`in` and merge-expression layout outside the deleted traceability roots. The formatter's proposed changes touched only that unchanged layout, so unrelated source formatting was preserved. This is not recorded as a passing formatting gate.

## Non-claims

These results do not prove provider truth, CI correctness, host sandboxing, global durability, distributed exactly-once execution, production availability, cutover readiness, or release eligibility. The failed operation is retained and must not be redispatched.
