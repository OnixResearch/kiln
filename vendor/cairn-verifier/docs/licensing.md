# Cairn licensing boundary

| Surface | License |
|---|---|
| `crates/cairn-core/**` | `MIT OR Apache-2.0` |
| `crates/cairn-gate-contract/**` | `MIT OR Apache-2.0` |
| `crates/cairn-radicle-cob/**` | `MIT OR Apache-2.0` |
| `templates/**` and reusable lifecycle examples | `MIT OR Apache-2.0` |
| Policy inputs and generated JSON named in [`cairn-policy/LICENSING.md`](../cairn-policy/LICENSING.md) | `AGPL-3.0-or-later OR MIT OR Apache-2.0`, including the specified historical revision |
| `crates/cairn-cli/**` | `AGPL-3.0-or-later` |
| Repository documentation and lifecycle material not carrying another notice | `AGPL-3.0-or-later` |
| Dependencies, generated artifacts containing upstream material, and vendored/reference code | Their stated upstream terms |

The permissive core and gate contract must not depend on `cairn-cli`. The CLI may depend on the core. Copying a reusable template does not by itself apply the CLI license to the generated project.

Complete texts are in `LICENSES/MIT.txt`, `LICENSES/Apache-2.0.txt`, and `LICENSES/AGPL-3.0-or-later.txt`. Each Cargo package directory carries the applicable text so package archives are self-contained.

The repository-owned `tools/check-license-boundary.rs` checks the declared grant,
its pinned revision, and each of the five covered policy paths and historical
blob identities through `license-policy.tsv`. These inventory checks do not
relicense other policy inputs, the exporter/toolchain or the CLI.

## Advisory prose dependency

The CLI links the `englishc` library and its `english` dependency from
[`Aaryan-Kapoor/english-rs`](https://github.com/Aaryan-Kapoor/english-rs), pinned
to revision `336ed696eec73e5b9858c5d7952b54c8e93b9df9`. Both packages are
MIT-licensed, copyright (c) 2026 Aaryan Kapoor. Their upstream
[`LICENSE`](https://github.com/Aaryan-Kapoor/english-rs/blob/336ed696eec73e5b9858c5d7952b54c8e93b9df9/LICENSE)
contains the applicable grant and notice; distributions containing copies or
substantial portions of that software must retain its copyright and permission
notice. This dependency is fetched from its immutable Git source, not vendored
or invoked as a runtime child process. The opt-in advisory prose check does not
change Cairn's licensing boundary or lifecycle acceptance.
The unchanged upstream notice is retained in
[`LICENSES/english-rs-LICENSE`](../LICENSES/english-rs-LICENSE) and installed by the
Nix package under `share/licenses/cairn/english-rs-LICENSE`.

Prior grants remain valid. This map is distribution metadata; it does not establish contributor authority, legal compliance, source correctness, lifecycle acceptance, or release eligibility.
