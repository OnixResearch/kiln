# Cairn licensing boundary

| Surface | License |
|---|---|
| `crates/cairn-core/**` | `MIT OR Apache-2.0` |
| `crates/cairn-gate-contract/**` | `MIT OR Apache-2.0` |
| `templates/**` and reusable lifecycle examples | `MIT OR Apache-2.0` |
| `crates/cairn-cli/**` | `AGPL-3.0-or-later` |
| Repository documentation and lifecycle material not carrying another notice | `AGPL-3.0-or-later` |
| Dependencies, generated artifacts containing upstream material, and vendored/reference code | Their stated upstream terms |

The permissive core and gate contract must not depend on `cairn-cli`. The CLI may depend on the core. Copying a reusable template does not by itself apply the CLI license to the generated project.

Complete texts are in `LICENSES/MIT.txt`, `LICENSES/Apache-2.0.txt`, and `LICENSES/AGPL-3.0-or-later.txt`. Each Cargo package directory carries the applicable text so package archives are self-contained.

Prior grants remain valid. This map is distribution metadata; it does not establish contributor authority, legal compliance, source correctness, lifecycle acceptance, or release eligibility.
