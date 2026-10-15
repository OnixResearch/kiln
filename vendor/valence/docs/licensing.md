# Valence licensing boundary

| Package or surface | License |
|---|---|
| `crates/valence-core/**` | `MPL-2.0` |
| `crates/valence-cli/**` | `AGPL-3.0-or-later` |
| `crates/valence-radicle/**` | `AGPL-3.0-or-later` |
| `pilots/shared-identity/**` | `AGPL-3.0-or-later` |
| `pilots/canonical-json-preserve-order/**` | `AGPL-3.0-or-later` |
| Repository documentation and lifecycle material not carrying another notice | `AGPL-3.0-or-later` |
| Dependencies, generated artifacts containing upstream material, and external references | Their stated upstream terms |

The core must not depend on CLI or pilot crates. Executable surfaces may depend on the MPL core. A future purely reusable pilot requires an explicit license review rather than silently inheriting the core license.

Complete texts are in `LICENSES/MPL-2.0.txt` and `LICENSES/AGPL-3.0-or-later.txt`. Each Cargo package directory carries the applicable text so package archives are self-contained.

`valence-core`, `valence-cli`, and `valence-radicle` are distributed from reviewed Git revisions and set `publish = false`. Their package names do not claim ownership of similarly normalized registry names. Any future registry release requires a separate package-name, ownership, compatibility, and release review.

Package licensing is distribution metadata and is outside canonical Evidence IR identity unless a versioned schema explicitly includes it. Prior grants and third-party terms remain intact. This map does not prove behavioral correctness, evidence truth, or release eligibility.
