# Cairn licensing boundary

| Surface | License |
|---|---|
| `crates/cairn-core/**` | `MIT OR Apache-2.0` |
| `crates/cairn-gate-contract/**` | `MIT OR Apache-2.0` |
| `crates/cairn-radicle-cob/**` | `MIT OR Apache-2.0` |
| `templates/**` and reusable lifecycle examples | `MIT OR Apache-2.0` |
| Policy inputs and generated JSON named in [`cairn-policy/LICENSING.md`](../cairn-policy/LICENSING.md) | `AGPL-3.0-or-later OR MIT OR Apache-2.0`, including the specified historical revision |
| `contracts/**` and `cairn-policy/assurance-targets.ncl` | `MIT OR Apache-2.0`, at every revision and for byte-for-byte copies |
| `crates/cairn-cli/**` | `AGPL-3.0-or-later` |
| Repository documentation and lifecycle material not carrying another notice | `AGPL-3.0-or-later` |
| Dependencies, generated artifacts containing upstream material, and vendored/reference code | Their stated upstream terms |

The permissive core and gate contract must not depend on `cairn-cli`. The CLI may depend on the core. Copying a reusable template does not by itself apply the CLI license to the generated project.

The Cairn copyright holders offer the consumer-facing typed Nickel contracts
under `contracts/**` and the assurance-targets policy source
`cairn-policy/assurance-targets.ncl` under a choice of `MIT OR Apache-2.0`.
This grant covers those paths at every revision of this repository, including
the copies other repositories vendored at earlier commits — Octet vendored
`contracts/assurance-targets.ncl` at
`3dc654d38404d595ff07ca994925789f45941ffe` and
`f20bcb4aa00268fdaa630cbce07633250f18a681` — and byte-for-byte copies of those
files in other repositories. Recipients may choose either of the listed
licenses for the covered files; changing or republishing an earlier Git commit
is not required. The grant does not cover the generated JSON exports (including
`cairn-policy/generated/assurance-targets.json`), other `cairn-policy/` files,
the CLI, the exporter/toolchain, repository documentation or third-party
material.

Complete texts are in `LICENSES/MIT.txt`, `LICENSES/Apache-2.0.txt`, and `LICENSES/AGPL-3.0-or-later.txt`. Each Cargo package directory carries the applicable text so package archives are self-contained.

The repository-owned `tools/check-license-boundary.rs` checks the declared grant,
its pinned revision, and each of the five covered policy paths and historical
blob identities through `license-policy.tsv`. It also checks the
consumer-contract grant's row, covered paths and vendored revisions. These
inventory checks do not relicense other policy inputs, generated JSON, the
exporter/toolchain or the CLI.

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
