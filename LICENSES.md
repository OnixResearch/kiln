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
  particular, the Cairn, Mantle, Nix-eval, and Radicle adapters link Bounded Exec;
  the Valence adapter links `valence-core`; the Pijul adapter links Pijul
  libraries. Their dependency obligations must also be considered.

## Integration and imported-source map

| Component and scope | Public terms and retained notices |
| --- | --- |
| `kiln-adapter-pijul` and its linked Pijul cohort | Adapter: `GPL-2.0-only`. `libpijul` and the Pijul fixture-producing executable at `1.0.0-beta.11` declare `GPL-2.0-or-later`; `pijul-repository` at `1.0.0-beta.10` declares the legacy identifier `GPL-2.0`. That legacy spelling is not a grant of later-version rights. Other cohort members retain their individual upstream terms. See [the Pijul adapter documentation](docs/pijul-source-adapter.md) and [GPL version 2](LICENSE-GPL-2.0). |
| Bounded Exec process libraries, revision `29dac88ecded94457572db3fdfaaaab95fa91525` | `bounded-exec` and `bounded-exec-core`: `AGPL-3.0-or-later`. Retained source and license: [`vendor/bounded-exec/`](vendor/bounded-exec/) and its [`LICENSE`](vendor/bounded-exec/LICENSE). These are linked Rust libraries, not merely an external executable. |
| Cairn gate contract and executable, revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f` | Linked `cairn-gate-contract`: `MIT OR Apache-2.0`, with [package license files](vendor/cairn/crates/cairn-gate-contract/). The separately executed `cairn-cli` is `AGPL-3.0-or-later`. The imported snapshot retains the upstream license material in [`vendor/cairn/`](vendor/cairn/). See [the Cairn adapter documentation](docs/cairn-gate.md) for the separate Bounded Exec linkage. |
| Lattice workflow contract and transport, revision `70496e67c7fd4a8b05914161a8e09de2759bebc8` | These imported packages declare `MIT`; their individual `LICENSE` files are retained under [`vendor/lattice/crates/lattice-workflow-contract/`](vendor/lattice/crates/lattice-workflow-contract/), including `transport/`. This describes the imported packages, not every component of Lattice. |
| Valence core, revision `220307a371e7bc4e3309447b09447d0fe6c4db5f` | Linked `valence-core`: `MPL-2.0`. Its [package license](vendor/valence/crates/valence-core/LICENSE) and upstream license material are retained in [`vendor/valence/`](vendor/valence/). The Kiln adapter's permissive declaration does not remove MPL obligations for covered files. |
| Artifact authentication core, imported transitively through Valence | `artifact-auth-core`: `MIT OR Apache-2.0`. Retained upstream license texts, [`NOTICE`](vendor/artifact/NOTICE), and [`provenance/SOURCE_GRANT.md`](vendor/artifact/provenance/SOURCE_GRANT.md) are in [`vendor/artifact/`](vendor/artifact/). Any source-grant scope remains exactly as recorded upstream; it is not a new grant for other imported material. |
| Mantle contract/fixture source, revision `0bbab77e23ab95caa4e4184b9945cfaa5c8040e2` | The imported build-contract schema and fixtures retain Mantle's upstream AGPL license in [`vendor/mantle/LICENSE`](vendor/mantle/LICENSE); no Mantle Rust producer code is imported. This is distinct from the license of Kiln's own Mantle adapter source and its linked Bounded Exec dependency. |
| Shared Git COB store, `onix-radicle-cob-git-store` version `0.2.0` | `MIT OR Apache-2.0`. Original [MIT](vendor/radicle-cob-git-store/LICENSE-MIT) and [Apache-2.0](vendor/radicle-cob-git-store/LICENSE-APACHE) texts are retained in the separate [`vendor/radicle-cob-git-store/`](vendor/radicle-cob-git-store/) import. |

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
