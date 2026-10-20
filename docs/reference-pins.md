# Design reference pins

The review date is 2026-08-08. These references inform bounded Kiln contracts and do not create runtime dependencies.

| Reference | Exact pin | License or specification | Selected concept | Rejected concept |
|---|---|---|---|---|
| Hydra | `07213d087c827266df136157d5a6c7ccd24db7ca` | GPL-3.0-or-later | CI vocabulary, evaluations, products, metrics, operator views | Monolithic control-plane authority |
| Radicle CI broker | RID `rad:zwTxygwuz5LDGBq255RA2CbNGrz8`, revision `a688afde1f730ef60402a809d9cdadc55e8dd749` | MIT OR Apache-2.0 | Node-local filtering and one-line JSON adapters | Broker database as Kiln state authority |
| Radicle native CI | RID `rad:z3qg5TKmN83afz2fj9z3fQjU8vaYE`, revision `f8353f411e6b97c78b988e4025f02013bdf4c522` | MIT OR Apache-2.0 | Two-response adapter lifecycle | Unisolated repository shell execution |
| Jujutsu | `786e65a1ee9648b2a72dc8c8f94d811833a17987` | Apache-2.0 | Change IDs, commit IDs, bookmarks, operations, divergence | Change IDs or revsets as immutable revisions |
| Choregraph | `2afbafb266c60530ae05a6862eb06ace72ae787a` | repository license | Pure bounded transitions | Execution authority in the graph core |
| Aspen | `08e4dfa5443737c81577f6691c673ab1ad78db4d` | repository license | Optional runtime shell | Runtime types in Kiln core |
| Mantle | `5dd6e8826e2934709e0b584ccf445a6fc5c57ba1` | repository license | Optional build observations | Builder authority transfer to Kiln |
| Nix | `2.34.8` executable identity `82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852` | LGPL-2.1-or-later | Exact local `.drv` execution and native JSON products | Flake registry, substitution, remote-builder, or build-authority transfer to Nix |
| Lattice | `5c9902f777412296c13e2347e45b3bede335b828` | repository license | Optional workflow execution | Workflow types as CI domain types |
| Valence | `b37177bbdd9ca0665412e6600f401e5cb54908d2` | repository license | Optional evidence projection | Evidence linkage as behavioral proof |
| Cairn | `837d730799faffe6a088cfd6dc49e08c7e0f1822` | repository license | Gates and six COB profiles | Lifecycle authority in Kiln |
| Zuul | release `12.0.0` | Apache-2.0 | Exact candidate sets and dependent queues | Gerrit, Git, executor, or merge authority |
| Concourse | `95872ce1a3af6fc0c4ab7f23658ec81f3e778d7c` | Apache-2.0 | Discovery, pinning, acquisition, mutation split | Universal opaque resources |
| Bazel Remote Execution API | `becdd8f9ff811df88a22d3eadd6341753d51d167`, API package `build.bazel.remote.execution.v2` | Apache-2.0 | CAS, action, operation, cache, and log identities | Protocol types in Kiln core |
| SLSA | Provenance `v1.1` | Community Specification | Provenance projection vocabulary | Automatic SLSA-level claims |
| in-toto | Statement `v1` | Apache-2.0 | Subject and predicate separation | Signer trust from schema validity |
| TUF | Specification `v1.0.31` | CC0-1.0 | Threshold, rollback, freeze, mirror threats | TUF metadata as Radicle governance |
| Kueue | API line `v1beta1` | Apache-2.0 | Admission and resource-fairness tests | Worker placement ownership |
| Dominant Resource Fairness | NSDI 2011 paper | Paper reference | Dominant-share invariant | Universal comparability of provider units |
| Temporal | Event-history compatibility model, review date 2026-08-08 | MIT | Replay compatibility checks | Temporal runtime dependency |
| CDEvents | Specification `v0.4.1` | Community Specification | CI event projection vocabulary | Imported events as canonical state |
| CloudEvents | Specification `v1.0.2` | Apache-2.0 | Event envelope binding | Envelope identity as Kiln identity |
| OpenTelemetry CI/CD | Semantic conventions development status, review date 2026-08-08 | Apache-2.0 | Rebuildable telemetry projections | Telemetry as lifecycle authority |
| Buildbot | `e055ebda6b4800b4762e45088b23b736e046cecf` | GPL-2.0-or-later | Source, scheduler, request, worker, reporter split | Buildbot runtime dependency |
| Pueue | `b3fe223ef0a3c51d95d5c6128700b79442c54af7` | MIT | Status, logs, wait, capacity groups, scoped controls | Scheduler, daemon, protocol, provider, or runtime integration |
| celld | release `v0.2.1`, revision `ae8fac053d79f971bfcb996054bb43eb2f9b05da` | Apache-2.0 | Durable Object SQLite transactions, one-writer fencing, conditional write, output gate | Workflow, build, placement, fairness, or release authority |

## Build source acquisition

The design-reference pins above are not the build cohort. The root Cargo
workspace consumes the following committed, immutable source imports; no sibling
checkout, GitHub SSH credential, private HTTPS credential, or running Radicle node
is a normal root workspace/default-package build input. The separately pinned
standalone runtimes and optional Octet gates have the acquisition boundary below.

| Import | Exact producer revision | Consumed scope | Source record |
|---|---|---|---|
| Lattice | `70496e67c7fd4a8b05914161a8e09de2759bebc8` | MIT workflow contract and Unix transport | [`vendor/lattice.source.json`](../vendor/lattice.source.json) |
| Cairn | `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f` | `MIT OR Apache-2.0` gate contract and pinned producer fixtures | [`vendor/cairn.source.json`](../vendor/cairn.source.json) |
| Valence | `220307a371e7bc4e3309447b09447d0fe6c4db5f` | MPL-2.0 `valence-core` | [`vendor/valence.source.json`](../vendor/valence.source.json) |
| Artifact | `e41340bec587b6d049b5cc518ec7db925dde84be` | Transitive `MIT OR Apache-2.0` `artifact-auth-core` | [`vendor/artifact.source.json`](../vendor/artifact.source.json) |
| Bounded Exec | `29dac88ecded94457572db3fdfaaaab95fa91525` | AGPL-3.0-or-later shell and core | [`vendor/bounded-exec.source.json`](../vendor/bounded-exec.source.json) |
| Mantle | `d75158b00b080535f2ffea12430665ca3d57ea82` | Build-contract producer fixtures and Nickel contract only | [`vendor/mantle.source.json`](../vendor/mantle.source.json) |
| UCAN | `d46816f19606b518307ba21ab33e8b93b6ebf646` | AGPL-3.0-or-later root shell and `ucan-nostr`; MPL-2.0 `ucan-core` and `verified-logic` snapshot | [`vendor/ucan.source.json`](../vendor/ucan.source.json) |

These are selected upstream Git-object archives, not copies of mutable working
trees. The imports preserve original source, manifests, license texts, and
applicable provenance notices byte-for-byte. Root workspace manifests retained
for inherited Cargo fields do not select the producer applications as dependencies.
The Artifact Git patch redirects Valence's exact transitive Git dependency to the
verified local import without changing Valence's upstream manifest. The root
workspace's generated lock has no Git sources. Its other Cargo dependencies use
the public crates.io registry and locked checksums; standalone runtime locks are
independent and retain their reviewed Git sources.

Run `cargo fmt -- --check` from the Kiln workspace root to check every admitted
workspace member, including the shared COB store. Do not use `--all`: that flag
also traverses local path dependencies and tries to load unimported applications
from retained producer workspace manifests. Producer snapshots are verified as
pristine imports, not reformatted or treated as complete upstream checkouts.

The Cairn and Mantle Nix cohort checks use these same committed source imports,
not separate private flake inputs. They still require the original exact revisions
and compare producer fixture and contract bytes against Kiln's frozen cohort.
The remaining flake inputs and their transitive inputs are public GitHub or
NixOS HTTPS archives pinned by the generated `flake.lock`.

The retained upstream repository URLs identify provenance, not a promise that
those URLs admit anonymous acquisition. The exact Cairn, Lattice, Valence, and
Artifact GitHub pins were unavailable anonymously during release preparation.
Bounded Exec's admitted read-only adapter returned HTTP 530; its RID remains
`rad:z2CpqLFpdP36fZXYUK5ZNWxMibpCo`. Kiln deliberately distributes its reviewed
source snapshot rather than adding an automatic mirror fallback or republishing
another project's repository. This does not change producer governance.

### Reproducing or updating an import

Normal builds need only the Kiln source archive. To reproduce an import, an
operator may supply any local Git object store containing the recorded commit:

```console
scripts/import-pinned-source.sh lattice "$LATTICE_SOURCE"
scripts/import-pinned-source.sh cairn "$CAIRN_SOURCE"
scripts/import-pinned-source.sh valence "$VALENCE_SOURCE"
scripts/import-pinned-source.sh artifact "$ARTIFACT_SOURCE"
scripts/import-pinned-source.sh bounded-exec "$BOUNDED_EXEC_SOURCE"
scripts/import-pinned-source.sh mantle "$MANTLE_SOURCE"
scripts/import-pinned-source.sh ucan "$UCAN_SOURCE"
```

The importer runs `git archive --format=tar COMMIT -- PATHS...` in the exact
`archive_paths` order recorded in each source JSON file, verifies its BLAKE3
against both the reviewed source record and importer pin, then imports it. If a
destination already exists, it compares bytes, symlink targets, and executable
bits and refuses replacement on drift. It does not inspect the checkout's HEAD
or working tree. Git, Bash, jq, BLAKE3's `b3sum`, GNU tar, and GNU coreutils are
import-time tools only.
Each source record additionally retains the BLAKE3 identity of the original full
`git archive --format=tar COMMIT` as `full_archive_blake3`. That is provenance
for the whole upstream revision, not an instruction to redistribute it. The
importer verifies only the explicitly selected archive; unrelated producer
applications and repository content are not imported.

A source update requires reviewing the new immutable revision, selected archive
paths, archive digest, upstream licenses and notices, and producer fixtures.
Update the source record and importer pin together. Review and remove the old
snapshot explicitly before installing a changed one; there is no force-update
mode. Regenerate `Cargo.lock` with Cargo and `flake.lock` with Nix, retain producer
cohort checks, and run the owning Kiln checks before claiming compatibility.
Do not hand-edit locks or add sibling-path dependencies.

For the initial Garden clone, use the command-local Git protocol setting in the
[quick start](../README.md#quick-start); it does not change global Git configuration.

For a fresh anonymous acquisition smoke, use an empty `HOME` and `CARGO_HOME`,
disable SSH and interactive Git credentials, then run `cargo fetch --locked` and
`nix flake archive --no-write-lock-file path:$PWD` with empty Nix `access-tokens`
and `secret-key-files`. This checks acquisition, not build correctness or release
eligibility. An offline metadata smoke may reuse only the public registry cache;
it must not reuse Cargo's Git cache.

### Registry execution transport cohort

`kiln-adapter-iroh` pins registry Iroh `0.95.1` (upstream Rust minimum 1.85)
under ALPN `kiln.workflow.v1`. Kiln retains Rust 1.88 and the existing imported
Lattice workflow contract/transport; Iroh does not enter provider-neutral cores.
Use the committed Cargo-generated lock and `--locked` commands. The signing
prereleases require their compatible published Iroh lock cohort:

| Registry package | Locked version |
|---|---|
| `ed25519` | `3.0.0-rc.1` |
| `pkcs8` | `0.11.0-rc.7` |
| `spki` | `0.8.0-rc.4` |
| `der` | `0.8.0-rc.9` |
| `pem-rfc7468` | `1.0.0-rc.3` |
| `signature` | `3.0.0-rc.4` |

Unconstrained resolution selected newer stable packages incompatible with
Iroh's `ed25519-dalek 3.0.0-pre.1`. The lock was corrected with Cargo, not local
registry edits or a patched cryptography implementation. Kiln maintainers must
recheck Rust 1.88 compilation, native peer/unknown-outcome conformance, and the
workspace gates when changing this cohort. This is compatibility evidence,
not a cryptographic audit or security guarantee.


### Standalone runtime and optional tool cohort

`runtime/kiln-aspen-host` and `runtime/kiln-radicle-nix-provider` remain independent
Cargo workspaces, excluded from the root workspace alongside the producer imports.
They retain their own manifests, locks, Rust toolchains, all-target/all-feature
package checks, and deny-all Octet gates. Selecting them is not a credential-free
build claim: their exact upstream repositories and private transitive producers
must be available to the operator. No sibling checkout or fallback is selected.

The host manifest and lock bind Kiln adapters to
`69afc2697f4330005797256135ae81b2722a125c`, Aspen/Molten to
`22f8ded26ca1907c29948e08b53f35df23080733`, Lattice contract/transport to
`70496e67c7fd4a8b05914161a8e09de2759bebc8`, and Durable File Publication to
`8e05e74e24b45f752d77145c4455385daaf6d6ab`. Both runtimes retain Bounded Exec
`29dac88ecded94457572db3fdfaaaab95fa91525` and the provider retains that same
Durable File Publication pin. These independent cohorts are not replaced by root
workspace imports merely because their package names overlap.

The optional Nix sources are isolated in
[`runtime/build-cohort/`](../runtime/build-cohort/), with a Nix-generated lock
preserving the canonical integration parent's producer pins and follows edges:

| Source | Exact revision | NAR identity |
|---|---|---|
| Octet | `cf04e894e53eb0947230118a086ef6066ddba38c` | `sha256-WMNrQL7l+LIabv5k4ekcN5Su5rXnaCYYyzzkNF2+NP0=` |
| Kamacite build support | `d76fe4abe543724d8fc0ac4b362187caf2e27622` | `sha256-Z6+dLkP2cziybjrAiRznP1cHUASyekhOwGl5ErWzZJc=` |

The root loads that content-addressed subflake only when an optional consumer
needs it. Octet's `nixpkgs` still follows the cohort's root `nixpkgs`, pinned to
`b7c2ada94fe99c15b0dbcf4d11fd7850b957a436`; the root asserts equality with its
own `nixpkgs` before selecting Octet. Other producer-owned transitive pins and
follows remain in the generated cohort lock. The Kamacite input supplies only its
exact compile-time `adapters/spacewasm-mvp/generated/profile.json` to the existing
writable build view. No producer source is imported or hand-edited by this
separation, and upstream licenses and notices still govern optional artifacts.

Normal `cargo fetch --locked`, `nix develop`, and default-package builds do not
select those optional sources. Full `nix flake check` does select the exported
runtime/Octet checks and therefore requires their sources. Unavailable sources
must fail that selected check, not turn it into a skip or success.

```console
nix flake archive --no-write-lock-file path:$PWD/runtime/build-cohort
cargo fetch --locked --manifest-path runtime/kiln-aspen-host/Cargo.toml
cargo fetch --locked --manifest-path runtime/kiln-radicle-nix-provider/Cargo.toml
nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L
nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-octet-deny-all --no-link -L
nix build path:$PWD#checks.x86_64-linux.kiln-radicle-nix-provider-package --no-link -L
nix build path:$PWD#checks.x86_64-linux.kiln-radicle-nix-provider-octet-deny-all --no-link -L
nix build path:$PWD#checks.x86_64-linux.aspen-runtime-octet-deny-all --no-link -L
nix build path:$PWD#checks.x86_64-linux.radicle-broker-wire-octet-deny-all --no-link -L
nix build path:$PWD#checks.x86_64-linux.octet --no-link -L
```

The final command builds all four admitted Octet consumer checks as one gate.

The retained Bounded Exec URL returned HTTP 530 during integration verification.
The declared same-RID projection
`https://seed.radicle.garden/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git` supplied exact
revision `29dac88ecded94457572db3fdfaaaab95fa91525` anonymously. Its full
Git-object archive matched the recorded BLAKE3
`4fbbf8f0749262469f00748e04c775180488dba800303f139172656d25931927`;
the observed source NAR was
`sha256-BVmqyUYyoNpY6LfOABxwPO3DY88ZtXeKNg3TPoGCcL0=`.
Kiln maintainers can reproduce that explicit acquisition into a disposable Nix
cache without changing product source URLs, revisions, or global Git configuration:

```console
cache_root=$(mktemp -d)
mkdir -p "$cache_root/empty-home"
export XDG_CACHE_HOME="$cache_root/cache"
env HOME="$cache_root/empty-home" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
  GIT_TERMINAL_PROMPT=0 GIT_CONFIG_COUNT=1 \
  GIT_CONFIG_KEY_0=url.https://seed.radicle.garden/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git.insteadOf \
  GIT_CONFIG_VALUE_0=https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git \
  nix eval --option access-tokens '' --option secret-key-files '' --json --expr \
  'builtins.fetchGit { url = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git"; rev = "29dac88ecded94457572db3fdfaaaab95fa91525"; allRefs = true; }'
nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L
```

The remaining private sources may use the operator's existing read credentials;
this procedure proves no anonymous access to those sources. The rewrite applies
only to the explicit cache-seeding command, not to subsequent builds. Source
acquisition is not a package, conformance, Octet, or deployment verdict.

To change this optional cohort, use Nix to regenerate its lock, review producer
pins/follows, then run `nix hash path runtime/build-cohort` and update the exact
subflake NAR identity in root `flake.nix`. Keep the root/cohort `nixpkgs` equality
and rerun the selected owning checks. Never hand-edit either lockfile.

## Maturity and non-claims

Hydra, Concourse, Buildbot, Jujutsu, and the Remote Execution API are mature references. Their maturity does not transfer to Kiln.

Radicle CI and its native adapter remain active ecosystem references. Passing compatibility fixtures does not prove node, broker, or runner correctness.

celld is the initial distributed event-store reference. A celld acknowledgement or storage-probe result does not prove object-store durability, hostile tenant isolation, or external effect execution.

The standards define interchange shapes. They do not grant source, builder, signer, gate, or release authority.
