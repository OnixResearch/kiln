# License map and third-party notices

Kiln is a mixed-license source tree. The workspace's `MIT OR Apache-2.0`
declaration is a default for Kiln-owned package source, **not** a license for
every dependency, imported file, integration, or compiled artifact. This notice
records scope; it does not replace the license texts, upstream notices, or
package-specific declarations.

## Kiln-owned source

- The Kiln packages under `crates/` that inherit `workspace.package.license`
  declare `MIT OR Apache-2.0`. Recipients may choose either license for that
  package's own source. See [LICENSE-MIT](LICENSE-MIT) and
  [LICENSE-APACHE](LICENSE-APACHE). Existing copyright notices remain unchanged.
- **Exception:** [`kiln-adapter-pijul`](crates/kiln-adapter-pijul/Cargo.toml)
  declares `GPL-2.0-only`. See [LICENSE-GPL-2.0](LICENSE-GPL-2.0).
- The permissive declaration on an adapter's own source does not relicense a
  linked dependency or establish the license of a combined executable. In
  particular, the Cairn, Patchgate, Mantle, Nix build, Nix-eval, Radicle, and GitHub adapters link Bounded Exec;
  the Valence adapter links `valence-core`; the Pijul adapter links Pijul
  libraries. Their dependency obligations must also be considered.

## Integration and imported-source map

| Component and scope | Public terms and retained notices |
| --- | --- |
| `kiln-adapter-pijul` and its linked Pijul cohort | Adapter: `GPL-2.0-only`. `libpijul` and the Pijul fixture-producing executable at `1.0.0-beta.11` declare `GPL-2.0-or-later`; `pijul-repository` at `1.0.0-beta.10` declares the legacy identifier `GPL-2.0`. That legacy spelling is not a grant of later-version rights. Other cohort members retain their individual upstream terms. See [the Pijul adapter documentation](docs/pijul-source-adapter.md) and [GPL version 2](LICENSE-GPL-2.0). |
| Bounded Exec process libraries, revision `29dac88ecded94457572db3fdfaaaab95fa91525` | `bounded-exec` and `bounded-exec-core`: `AGPL-3.0-or-later`. Retained source and license: [`vendor/bounded-exec/`](vendor/bounded-exec/) and its [`LICENSE`](vendor/bounded-exec/LICENSE). These are linked Rust libraries, not merely an external executable. |
| UCAN observation-auth cohort, revision `d46816f19606b518307ba21ab33e8b93b6ebf646` | Vendored root `ucan` shell and `ucan-nostr`: `AGPL-3.0-or-later`; `ucan-core` and the immutable `verified-logic` snapshot: `MPL-2.0`. Retained license texts are in [`vendor/ucan/LICENSES/`](vendor/ucan/LICENSES/). The HTTP observation adapter links this cohort for the optional UCAN holder-invocation auth backend. |
| Cairn gate contract and executable, revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f` | Linked `cairn-gate-contract`: `MIT OR Apache-2.0`, with [package license files](vendor/cairn/crates/cairn-gate-contract/). The separately executed `cairn-cli` is `AGPL-3.0-or-later`. The imported snapshot retains the upstream license material in [`vendor/cairn/`](vendor/cairn/). See [the Cairn adapter documentation](docs/cairn-gate.md) for the separate Bounded Exec linkage. |
| Lattice workflow contract and transport, revision `70496e67c7fd4a8b05914161a8e09de2759bebc8` | These imported packages declare `MIT`; their individual `LICENSE` files are retained under [`vendor/lattice/crates/lattice-workflow-contract/`](vendor/lattice/crates/lattice-workflow-contract/), including `transport/`. This describes the imported packages, not every component of Lattice. |
| Valence core, revision `220307a371e7bc4e3309447b09447d0fe6c4db5f` | Linked `valence-core`: `MPL-2.0`. Its [package license](vendor/valence/crates/valence-core/LICENSE) and upstream license material are retained in [`vendor/valence/`](vendor/valence/). The Kiln adapter's permissive declaration does not remove MPL obligations for covered files. |
| Artifact authentication core, imported transitively through Valence | `artifact-auth-core`: `MIT OR Apache-2.0`. Retained upstream license texts, [`NOTICE`](vendor/artifact/NOTICE), and [`provenance/SOURCE_GRANT.md`](vendor/artifact/provenance/SOURCE_GRANT.md) are in [`vendor/artifact/`](vendor/artifact/). Any source-grant scope remains exactly as recorded upstream; it is not a new grant for other imported material. |
| Mantle contract/fixture source, revision `d75158b00b080535f2ffea12430665ca3d57ea82` | The imported build-contract schema and fixtures retain Mantle's upstream AGPL license in [`vendor/mantle/LICENSE`](vendor/mantle/LICENSE); no Mantle Rust producer code is imported. This is distinct from the license of Kiln's own Mantle adapter source and its linked Bounded Exec dependency. |
| `kiln-adapter-nix` and the separately executed Nix CLI | Kiln adapter source: `MIT OR Apache-2.0`; its linked Bounded Exec dependency is `AGPL-3.0-or-later`. The pinned Nix `2.34.8` executable is a separately executed program with its own `LGPL-2.1-or-later` terms. No Nix source or library is imported into Kiln. See [the Nix build adapter documentation](docs/nix-build-adapter.md). |
| `kiln-adapter-nix-eval` and stock `nix-eval-jobs` | Kiln adapter source: `MIT OR Apache-2.0`; linked Bounded Exec: `AGPL-3.0-or-later`. The separately executed stock evaluator at `NixOS/nix-eval-jobs@a0cd02231c58974a6b5aaa3712069b071047162e` declares `GPL-3.0`; retain that exact declaration rather than infer later-version rights. Its Nix libraries retain their own terms. The adapter does not link Nix libraries into Kiln core. See [the discovery adapter](docs/nix-eval-jobs-adapter.md). |
| Optional Onix discovery fixture and runtime cohort | Imported fixture: `AGPL-3.0-or-later` SPDX declaration. Separately executed evaluator: recorded `GPL-3.0`; fork: `LGPL-2.1-or-later`; vendored Nickel: `MIT`. Plugin and onixpkgs wrapper license status remains `NOASSERTION`, not an inferred permissive grant. See the exact provenance below. |
| Shared Git COB store, `onix-radicle-cob-git-store` version `0.2.0` | `MIT OR Apache-2.0`. Original [MIT](vendor/radicle-cob-git-store/LICENSE-MIT) and [Apache-2.0](vendor/radicle-cob-git-store/LICENSE-APACHE) texts are retained in the separate [`vendor/radicle-cob-git-store/`](vendor/radicle-cob-git-store/) import. |
| GitHub service and exact-source job (`kiln-adapter-github`, `kiln-github-job`) | Kiln-owned source inherits `MIT OR Apache-2.0`; both mechanisms use linked Bounded Exec (`AGPL-3.0-or-later`), not just a separately executed tool. SecretSpec is separately executed under its own upstream terms. The adapter's permissive metadata is not a permissive-only license claim for the combined binaries. |
| `kiln-adapter-patchgate` and `kiln-patchgate-review` | Kiln-owned source inherits `MIT OR Apache-2.0`; linked Bounded Exec is `AGPL-3.0-or-later`, so combined artifacts are not permissive-only. Patchgate and its patched Pijul are separately installed executables, not linked libraries or default acquired sources. Operators must retain their selected package and runtime-closure notices; an executable hash is not a source-provenance or license grant. |
| Iroh workflow transport, registry version `0.95.1` | Iroh and Kiln's adapter declare `MIT OR Apache-2.0`; imported Lattice workflow packages retain `MIT`. Registry cryptography, QUIC, TLS and native dependencies retain their individual terms. The generated root lock binds the selected cohort; this row is not a permissive-only claim for every combined workspace artifact. |
| Property-based terminal rail tool (Bombadil), revision `832faf4d953e2c56d1d901aca1bb2c3c3170221d` | Separately executed test tool: `MIT`. It observes the packaged `kiln-dashboard` binary under a pseudo terminal and is never linked into a Kiln package. No Bombadil code, crate, or library enters the Kiln source tree or any Kiln artifact. See [the operator dashboard documentation](docs/observation-dashboard.md). |
| Offline browser index | Kiln-owned browser source: `MIT OR Apache-2.0`. Pinned Subwayrat `rat-table` at `2e52b3150819a2365aaefd3dcf8bbd2a2fa2e901`: `MIT`, with pristine [LICENSE](vendor/subwayrat/LICENSE) and [LICENSING.md](vendor/subwayrat/LICENSING.md). Ratzilla 0.3.1: `MIT OR Apache-2.0`; the MIT option is retained. The isolated [Cargo.lock](web/dashboard-index/Cargo.lock) binds registry checksums. Original dependency notices are embedded in each catalog under **Browser index licenses** and retained in [the generated notice bundle](crates/kiln-dashboard/src/render/generated/rats-index-notices.txt). Archives omitting license files use the exact upstream-revision MIT texts under [`web/dashboard-index/notices/`](web/dashboard-index/notices/), with URLs in the generator and bundle. This metadata cohort includes build tools and optional dependency edges; it is not an assertion that every listed package is linked. |
| OnixOS pilot coordinator and worker (`kiln-onixos-pilot`, `kiln-onixos-worker`) | Kiln-owned source inherits `MIT OR Apache-2.0`. The coordinator links the UCAN observation cohort (`AGPL-3.0-or-later` / `MPL-2.0`); the worker links the Nix adapters and Bounded Exec (`AGPL-3.0-or-later`). Their combined artifacts are not permissive-only. The separately executed maintained Aspen/Lattice and Onix evaluator/plugin programs retain their own terms and exact source cohorts; the plugin wrapper's `NOASSERTION` remains unresolved. The selected OnixOS consumer and explicit shim substitution do not transfer a license grant to Kiln. |
| Radicle source-to-build controller (`kiln-radicle-ci`) | Kiln-owned source inherits `MIT OR Apache-2.0`. It links the existing pilot/UCAN observation cohort (`AGPL-3.0-or-later` / `MPL-2.0`) and source/status adapters; the existing project worker links the Nix adapters and Bounded Exec (`AGPL-3.0-or-later`). These combined executables are not permissive-only. Separately executed Radicle, SecretSpec, Aspen/Lattice and evaluator programs retain their own pinned terms. The isolated native source/status fixtures grant no production repository or signing authority. |

See [build source acquisition](docs/reference-pins.md#build-source-acquisition)
for exact import records and drift-refusing acquisition. Imported root Cargo
manifests preserve inherited package fields; they do not import or select all
upstream applications.

This is the scoped map for the integrations above, not an exhaustive inventory
of all registry dependencies, native libraries, Nix build inputs, or separately
installed provider programs. Their own license files and notices remain
applicable. Check the actual selected dependency graph and shipped contents,
including native code and any locally patched dependency source, when preparing
a source or binary distribution. Preserving an immutable upstream pin does not
waive obligations for modifications applied during a build.

## Optional Onix discovery provenance

The explicit `onix-nix-eval-v1` profile does not relicense stock discovery or
change the workspace default. Its sources are acquired separately through
[`runtime/onix-eval-cohort/flake.nix`](runtime/onix-eval-cohort/flake.nix) and its
generated lock, including the exact onixpkgs SSH review-source input. Default
core/workspace builds and stock discovery do not acquire that optional cohort.
Kiln runs the evaluator as a process; it does not import the evaluator/fork
libraries into core. Bounded Exec remains linked into the Kiln adapter under
its AGPL terms regardless of the selected external evaluator.

| Exact source and scope | Observed/recorded declaration and limit |
|---|---|
| OnixOS `fixtures/kiln-wasm-discovery/flake.nix`, imported verbatim at [`runtime/onix-eval-cohort/jobset/flake.nix`](runtime/onix-eval-cohort/jobset/flake.nix) | The file declares `SPDX-License-Identifier: AGPL-3.0-or-later`. Its dedicated immutable source commit and original Git-object path/content identity are retained in [`jobset.source.json`](runtime/onix-eval-cohort/jobset.source.json). This imported producer file is not covered by Kiln's permissive workspace default, nor is this declaration a license finding for all of OnixOS. |
| Evaluator `NixOS/nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a`, package version `2.36.4` | The selected package/manifest records `GPL-3.0`. Preserve this spelling; this notice does not invent a later-version grant. The package applies two measured fork-2.36 source patches recorded in [`manifest.nix`](runtime/onix-eval-cohort/manifest.nix); distributing a modified executable requires assessing its actual corresponding-source and notice obligations. |
| Evaluator-linked fork `onixcomputer/nix@f93f28ad9dce007baae13aa461c16abf656d1171` | The selected provenance records `LGPL-2.1-or-later`. The seven measured `libnix*.so.2.36.0` files belong to the evaluator closure, not the independently built OnixOS CLI closure. This is not an exhaustive license inventory of native/transitive libraries. |
| Plugin `onixcomputer/onix-wasm@54bf09574c42a21f9f38dc7e01bc44440e6fbc07` | Its pinned [`nickel-plugin/Cargo.toml`](https://github.com/onixcomputer/onix-wasm/blob/54bf09574c42a21f9f38dc7e01bc44440e6fbc07/nickel-plugin/Cargo.toml) declares package name/version and dependencies but no package license. The manifest deliberately records `NOASSERTION (plugin wrapper); vendored Nickel MIT`. Vendored Nickel's MIT terms do not establish a license grant for the wrapper or the entire WASM artifact. |
| Nickel WASM vendor `brittonr/nickel-wasm@5de51b8976cf6a35ae8d026a4f28916df5f259fa`, based on `nickel-lang/nickel@1320a983e6c3d1e2fb53dd2464b084b4903b1426` | Its pinned [`Cargo.toml`](https://github.com/brittonr/nickel-wasm/blob/5de51b8976cf6a35ae8d026a4f28916df5f259fa/Cargo.toml) declares workspace version `1.17.0` and `license = "MIT"`. The retained fixture identifies both producer revisions and the optional lock pins the vendor source. Preserve upstream license/copyright material and separately assess all actual shipped dependencies. |
| Package wrapper `OnixResearch/onixpkgs@3bdf51c00947ffd6b8bd3355ff83509a03752042` | The manifest records `NOASSERTION (package wrapper); evaluator GPL-3.0`. A package's evaluator license does not establish the wrapper's license. This is a review-source pin, not a production release or evidence of publication to main/Radicle. |

`NOASSERTION` means no wrapper grant is asserted by this notice. It is not public
domain, permissive licensing, a compatibility determination, or authorization
to redistribute an otherwise unlicensed component. Resolve missing grants and
preserve applicable license texts/notices before distribution. Exact hashes,
successful pure discovery, and owner approval of a bounded source-review
exercise do not supply additional license rights or establish release readiness.

## External programs, linked libraries, and combined outputs

Invoking an independently installed executable through its documented process
protocol is different from incorporating a library into a compiled program.
For example, the Cairn CLI is invoked as a process, while the Bounded Exec
process mechanism is linked into the Kiln adapter. Calling that linked library
a process boundary does not remove its AGPL terms. Distributing an external
executable still requires satisfying that executable's own license obligations;
a process boundary is not a blanket license exception either.

For GPL/AGPL-covered distributions, assess the applicable notices, license
copies, corresponding source, and other conditions for the actual work being
conveyed. AGPL also has requirements for modified covered programs that support
remote network interaction; consult section 13 of the license. For MPL-covered
files, preserve the notices and meet the covered-source availability and other
conditions when distributing source or executable forms. These summaries are
not substitutes for the license texts.

Do not assume that the GPL-2.0-only Pijul integration and AGPL-3.0-or-later
integrations can be combined under one license. This notice does not certify
license compatibility of every workspace artifact or grant additional
permissions. It also does **not** relicense unrelated permissive packages,
require whole-repository relicensing, or assert a ban on public source
publication. Assess each distributed combination under its applicable terms.

## Owner-specific source reuse

The Cairn integration documentation records OnixResearch owner-authorized
source reuse for Bounded Exec. That grant does **not** change Bounded Exec's
public `AGPL-3.0-or-later` license, cover third-party material, or apply to other
recipients. Public recipients must not infer a permissive exception from
Kiln's workspace declaration, the adapter's metadata, this notice, or that
owner-specific authorization.

## Prospective owned-dependency licensing

The operator selected `AGPL-3.0-or-later` for new owned-source releases of the
dependencies below, including their reusable cores, contracts, schemas, owned
templates, fixtures, and policy. Each producer owns its authoritative license
map and preparation checks. Kiln maintainers own the selected dependency
inventory and the boundary between preparation and exact consumed revisions.

| Owning repository | Selected dependency scope |
| --- | --- |
| Artifact (`onix-artifact`) | Authentication/binding packages and transitive imported authentication core |
| Aspen/Molten | Standalone host runtime and native service composition |
| Basalt | Transitive runtime policy dependency |
| Bounded Exec | Linked process shell and core |
| Cairn | Gate contract, runtime core, and selected build-tool inputs |
| ChaosControl | Runtime snapshot descriptor |
| Choregraph | Runtime branchable history |
| Content Identity Core | Transitive runtime content identity |
| Durable File Publication | Standalone runtime publication mechanism |
| Kamacite | Runtime interchange core and exact optional build profile |
| Lattice | Workflow contract and Unix transport |
| Mantle | Imported build contract and fixtures |
| Nickel Export | Transitive optional build-tool input |
| Octet | Optional source gate and its historical Slotcar inputs |
| Radicle COB Git Store | Separately imported shared store |
| Schema Identity Core | Runtime schema identities |
| Schema Migration Core | Runtime schema migration decisions |
| Trellis | Original owner of the verified-logic snapshot carried by UCAN |
| UCAN | Runtime capability library and its immutable verified-logic import |
| Valence | Linked evidence core |

This is prospective source preparation, **not** a relabeling of the earlier
revisions in the integration map above. Existing MIT, Apache, MPL, and
multilicensing grants remain valid, including previously granted byte-identical
content. Third-party notices and immutable imported snapshots retain their
original terms. Kiln's current source imports, locks, package licenses, and
runtime cohorts are unchanged; moving a consumer to a future AGPL release needs
its own exact-revision, compatibility, and combined-output review.

The historical Slotcar commits `2f8213a88781d9d38bb81319d06a50805d48ccd3` and
`b18a547801dfebaa5ee3e748e3ebefed1a66cb17` are present in Octet's source history;
Octet's README identifies Slotcar as its historical name. They do not identify a
separate missing implementation owner. Historical URLs still need their own
acquisition evidence.

All twenty selected owners now have local prospective AGPL source preparation:
fifteen producer changes and five owners that already declared AGPL. Trellis's
full-policy admission repair preserves all pre-existing obligations, and its
licensing, packaging, native gates, Verus and coverage checks passed locally.
Its licensing change remains open because native accepted-spec sync rejects
pre-existing multiple requirement IDs per block; no broader legacy migration or
archive was performed. Producer Nix check groups remain blocked on store
installation, and Lattice's retained Rats source identity and notice review
remain open. Those failures are not passes and are not repaired here.

These producer prerequisites are **not** a requirement to migrate Kiln's
unchanged source imports to new AGPL releases before publishing Kiln source.
Kiln imports only Lattice's workflow contract and transport, not its Rats
snapshot. Trellis is a nested source of the separately pinned UCAN runtime
cohort, not a vendored Kiln source import. A separately authorized producer
remote is not required to redistribute the already imported shared COB store
under its retained public grant. None of these distinctions certifies rights
for a future producer release, standalone runtime closure, or combined binary.

The [current-source review](.cairn/archive/1970-01-01-prepare-owned-dependencies-agpl/evidence/current-source-review.json)
records retained grants, notices and the applicability of each prerequisite.
The [producer verification receipt](.cairn/archive/1970-01-01-prepare-owned-dependencies-agpl/evidence/final-verification.json)
and [executed command record](.cairn/archive/1970-01-01-prepare-owned-dependencies-agpl/evidence/commands.json)
retain the actual target roots and all observed failures. An exact Kiln source
candidate still needs its own anonymous acquisition, privacy/rights and
distribution-scope review. This inventory does not prove copyright title,
third-party permission, AGPL compliance of every shipped combination, or
authority to publish.

## Included license texts and provenance

- [LICENSE-GPL-2.0](LICENSE-GPL-2.0) is the verbatim GPL version 2 text retrieved
  from the Free Software Foundation's official
  [`https://ftp.gnu.org/gnu/Licenses/gpl-2.0.txt`](https://ftp.gnu.org/gnu/Licenses/gpl-2.0.txt).
  SHA-256: `edaef632cbb643e4e7a221717a6c441a4c1a7c918e6e4d56debc3d8739b233f6`.
- [LICENSE-AGPL-3.0](LICENSE-AGPL-3.0) is copied verbatim from Bounded Exec's
  `LICENSE` at revision `29dac88ecded94457572db3fdfaaaab95fa91525`, also retained
  as [`vendor/bounded-exec/LICENSE`](vendor/bounded-exec/LICENSE).
  SHA-256: `0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0`.
  Including version 3's text does not remove an upstream `-or-later` grant.
- The MPL-2.0 text and imported permissive license texts are retained with the
  relevant imports linked above. Preserve all applicable upstream copyright,
  attribution, `NOTICE`, license, and provenance files when copying imports.

No new copyright attribution, exception, or relicensing grant is created by
this map. Package-specific license notices and the applicable license terms
control; inclusion in this repository is not a replacement for them.
